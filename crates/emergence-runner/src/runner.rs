//! Agent decision runner: the core pipeline from perception to action.
//!
//! Orchestrates the full decision loop per `agent-system.md` section 6.1:
//! 1. Receive perception payload
//! 2. Check rule engine for routine actions (fast-path bypass)
//! 3. Score decision complexity (task 6.2.2)
//! 4. Select LLM backend based on complexity (task 6.2.3)
//! 5. Render prompt from templates
//! 6. Call LLM backend (with timeout and fallback)
//! 7. Parse structured action from response
//! 8. Submit action to World Engine via NATS
//!
//! Timeout handling ensures an agent never misses a tick -- if the LLM
//! call exceeds the deadline, a `NoAction` is submitted immediately.
//!
//! The rule engine (task 6.2.1) and night cycle optimization (task 6.2.4)
//! bypass the LLM entirely for obvious survival decisions, reducing cost
//! and latency for routine ticks.
//!
//! When complexity routing is enabled (task 6.2.3), high-complexity
//! decisions are routed to the escalation backend first, while
//! low/medium decisions use the cheap primary backend.

use std::time::{Duration, Instant};

use chrono::Utc;
use emergence_types::{ActionParameters, ActionRequest, ActionType, AgentId, DecisionRecord, Perception};
use futures::StreamExt;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::complexity::{score_complexity, ComplexityLevel};
use crate::containment;
use crate::error::RunnerError;
use crate::llm::LlmBackend;
use crate::nats::NatsClient;
use crate::parse::parse_llm_response;
use crate::prompt::PromptEngine;
use crate::rule_engine::{self, DecisionSource};

/// Maximum length for the raw LLM response stored in a [`DecisionRecord`].
const MAX_RAW_RESPONSE_LEN: usize = 4000;

/// Maximum length for the prompt stored in a [`DecisionRecord`].
const MAX_PROMPT_LEN: usize = 8000;

/// Metadata captured from an LLM decision for the [`DecisionRecord`].
struct LlmDecisionMeta {
    /// The rendered prompt (system + user) sent to the LLM.
    prompt_sent: String,
    /// The raw text response from the LLM.
    raw_response: String,
    /// Which backend answered (e.g. `"openai-compatible"`, `"anthropic"`).
    backend_name: String,
    /// Wall-clock latency of the LLM call in milliseconds.
    latency_ms: u64,
}

/// The agent decision runner.
///
/// Holds references to all components needed for the decision pipeline:
/// NATS client, prompt engine, LLM backends (primary + optional
/// escalation), and configuration flags for rule engine bypass and
/// complexity-based routing.
pub struct AgentRunner {
    nats: NatsClient,
    prompt_engine: PromptEngine,
    primary_backend: LlmBackend,
    escalation_backend: Option<LlmBackend>,
    decision_timeout: Duration,
    /// When true, the rule engine checks for obvious survival actions
    /// before calling the LLM.
    routine_action_bypass: bool,
    /// When true, agents at night with low energy or no activity
    /// auto-rest without an LLM call.
    night_cycle_skip: bool,
    /// When true, high-complexity decisions are routed to the escalation
    /// backend first instead of the primary backend.
    complexity_routing_enabled: bool,
    /// This runner's partition ID (0-indexed).
    partition_id: u32,
    /// Total number of runner partitions.
    total_partitions: u32,
}

impl AgentRunner {
    /// Create a new agent runner with all required components.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        nats: NatsClient,
        prompt_engine: PromptEngine,
        primary_backend: LlmBackend,
        escalation_backend: Option<LlmBackend>,
        decision_timeout: Duration,
        routine_action_bypass: bool,
        night_cycle_skip: bool,
        complexity_routing_enabled: bool,
    ) -> Self {
        Self {
            nats,
            prompt_engine,
            primary_backend,
            escalation_backend,
            decision_timeout,
            routine_action_bypass,
            night_cycle_skip,
            complexity_routing_enabled,
            partition_id: 0,
            total_partitions: 1,
        }
    }

    /// Set the partition configuration for multi-runner mode.
    ///
    /// When `total_partitions > 1`, this runner only processes agents
    /// where `hash(agent_id) % total_partitions == partition_id`.
    pub const fn with_partitioning(mut self, partition_id: u32, total_partitions: u32) -> Self {
        self.partition_id = partition_id;
        self.total_partitions = total_partitions;
        self
    }

    /// Run the main decision loop.
    ///
    /// Subscribes to perception messages from NATS and processes each one
    /// through the decision pipeline. This method runs indefinitely until
    /// the NATS connection drops or an unrecoverable error occurs.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError`] if NATS subscription fails.
    pub async fn run(&self) -> Result<(), RunnerError> {
        let mut subscriber = self.nats.subscribe_perceptions().await?;
        info!(
            partition_id = self.partition_id,
            total_partitions = self.total_partitions,
            "agent runner started, awaiting perception payloads"
        );

        while let Some(message) = subscriber.next().await {
            let subject = message.subject.to_string();
            let tick = NatsClient::extract_tick_from_subject(&subject).unwrap_or(0);

            debug!(
                subject = subject,
                tick = tick,
                payload_size = message.payload.len(),
                "received perception message"
            );

            match NatsClient::deserialize_perception(&message.payload) {
                Ok(perception) => {
                    let agent_id = perception.self_state.id;

                    // Multi-runner partitioning: skip agents that belong to
                    // other runner instances.
                    if !NatsClient::is_my_agent(
                        &agent_id,
                        self.partition_id,
                        self.total_partitions,
                    ) {
                        debug!(
                            agent_id = %agent_id,
                            partition_id = self.partition_id,
                            "skipping agent (not my partition)"
                        );
                        continue;
                    }

                    let action = self.decide(tick, &perception).await;
                    if let Err(e) = self.nats.publish_action(tick, &action).await {
                        warn!(
                            agent_id = %agent_id,
                            tick = tick,
                            error = %e,
                            "failed to publish action"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        subject = subject,
                        error = %e,
                        "failed to deserialize perception, skipping"
                    );
                }
            }
        }

        info!("NATS subscription ended, runner shutting down");
        Ok(())
    }

    /// Execute the full decision pipeline for a single agent tick.
    ///
    /// First checks the rule engine for routine actions (fast-path) and
    /// night cycle optimization. If neither applies, falls through to the
    /// LLM pipeline with timeout. If the deadline is exceeded, returns a
    /// `NoAction` request so the agent does not miss the tick.
    ///
    /// After each decision (regardless of source), publishes a
    /// [`DecisionRecord`] to NATS for the Observer dashboard.
    async fn decide(&self, tick: u64, perception: &Perception) -> ActionRequest {
        let agent_id = perception.self_state.id;

        // Fast-path: night cycle optimization (task 6.2.4)
        // Checked first because sleeping agents should not even reach the
        // routine action rules -- they just rest.
        if self.night_cycle_skip
            && let Some(action) = rule_engine::try_night_cycle_rest(perception)
        {
            info!(
                agent_id = %agent_id,
                tick = tick,
                action_type = ?action.action_type,
                decision_source = DecisionSource::NightCycle.as_str(),
                "decision bypassed LLM (night cycle)"
            );
            self.publish_decision_record(
                &action,
                DecisionSource::NightCycle,
                None,
                Some("night_rest"),
            );
            return action;
        }

        // Fast-path: routine action bypass (task 6.2.1)
        if self.routine_action_bypass
            && let Some(action) = rule_engine::try_routine_action(perception)
        {
            info!(
                agent_id = %agent_id,
                tick = tick,
                action_type = ?action.action_type,
                decision_source = DecisionSource::RuleEngine.as_str(),
                "decision bypassed LLM (routine action)"
            );
            self.publish_decision_record(
                &action,
                DecisionSource::RuleEngine,
                None,
                Some(&format!("{:?}", action.action_type)),
            );
            return action;
        }

        // If we get here, the LLM is making the decision -- reset loop detection.
        rule_engine::reset_loop_detection(agent_id);

        // Full LLM pipeline with timeout
        match timeout(self.decision_timeout, self.decide_inner(tick, perception)).await {
            Ok(Ok((action, meta))) => {
                debug!(
                    agent_id = %agent_id,
                    tick = tick,
                    decision_source = DecisionSource::Llm.as_str(),
                    "decision made via LLM"
                );
                self.publish_decision_record(
                    &action,
                    DecisionSource::Llm,
                    Some(&meta),
                    None,
                );
                action
            }
            Ok(Err(e)) => {
                warn!(
                    agent_id = %agent_id,
                    tick = tick,
                    error = %e,
                    "decision pipeline failed, submitting NoAction"
                );
                let action = no_action_request(agent_id, tick);
                self.publish_decision_record(
                    &action,
                    DecisionSource::Llm,
                    None,
                    None,
                );
                action
            }
            Err(_) => {
                warn!(
                    agent_id = %agent_id,
                    tick = tick,
                    timeout_ms = self.decision_timeout.as_millis(),
                    "decision deadline exceeded, submitting NoAction"
                );
                let action = no_action_request(agent_id, tick);
                self.publish_decision_record_with_source(
                    &action,
                    "timeout",
                    None,
                    None,
                );
                action
            }
        }
    }

    /// Inner decision logic (without timeout wrapper).
    ///
    /// 1. Score decision complexity
    /// 2. Serialize perception and render prompt
    /// 3. Call LLM with complexity-aware backend routing
    /// 4. Parse response into action
    ///
    /// Returns the action request and LLM metadata for the decision record.
    async fn decide_inner(
        &self,
        tick: u64,
        perception: &Perception,
    ) -> Result<(ActionRequest, LlmDecisionMeta), RunnerError> {
        let agent_id = perception.self_state.id;

        // Step 1: Score complexity
        let complexity = score_complexity(perception);

        debug!(
            agent_id = %agent_id,
            tick = tick,
            complexity = %complexity,
            "decision complexity scored"
        );

        // Step 2: Serialize perception to JSON for template rendering
        let perception_json = serde_json::to_value(perception)?;

        // Step 3: Render prompt
        let prompt = self.prompt_engine.render(&perception_json)?;

        // Step 4: Call LLM with complexity-aware backend selection
        let start = Instant::now();
        let (raw_response, backend_name) =
            self.call_with_routing(agent_id, complexity, &prompt)
                .await?;
        // LLM calls take at most a few seconds; millis will never exceed u64.
        #[allow(clippy::cast_possible_truncation)]
        let latency_ms = start.elapsed().as_millis() as u64;

        // Step 5: Containment scan (Phase 5.4.2)
        let containment_result = containment::scan_response(&raw_response);
        if containment_result.threats_detected {
            warn!(
                agent_id = %agent_id,
                tick = tick,
                threat_count = containment_result.findings.len(),
                "containment: threats detected in LLM response for agent"
            );
        }

        // Step 6: Parse the response
        let decision = parse_llm_response(&raw_response);

        // Step 7: Scan communication messages for exploitation (Phase 5.4.3)
        if let ActionParameters::Communicate { ref message, .. }
        | ActionParameters::Broadcast { ref message } = decision.parameters
        {
            let msg_scan = containment::scan_message(message);
            if msg_scan.threats_detected {
                warn!(
                    agent_id = %agent_id,
                    tick = tick,
                    threat_count = msg_scan.findings.len(),
                    "containment: threats detected in agent communication message"
                );
            }
        }

        info!(
            agent_id = %agent_id,
            tick = tick,
            complexity = %complexity,
            action_type = ?decision.action_type,
            reasoning = ?decision.reasoning,
            latency_ms = latency_ms,
            "decision parsed"
        );

        let prompt_text = format!("{}\n\n{}", prompt.system, prompt.user);

        let meta = LlmDecisionMeta {
            prompt_sent: truncate_string(&prompt_text, MAX_PROMPT_LEN),
            raw_response: truncate_string(&raw_response, MAX_RAW_RESPONSE_LEN),
            backend_name,
            latency_ms,
        };

        Ok((
            ActionRequest {
                agent_id,
                tick,
                action_type: decision.action_type,
                parameters: decision.parameters,
                submitted_at: Utc::now(),
            },
            meta,
        ))
    }

    /// Call the LLM with complexity-aware backend routing and fallback.
    ///
    /// When complexity routing is **enabled** and an escalation backend
    /// is configured:
    ///
    /// - `Low` / `Medium` complexity: primary -> escalation -> error
    /// - `High` complexity: escalation -> primary -> error
    ///
    /// When complexity routing is **disabled** (or no escalation backend):
    ///
    /// - All complexity levels: primary -> escalation -> error
    ///
    /// The fallback chain always tries both backends before giving up.
    ///
    /// Returns the raw response text and the name of the backend that responded.
    async fn call_with_routing(
        &self,
        agent_id: AgentId,
        complexity: ComplexityLevel,
        prompt: &crate::prompt::RenderedPrompt,
    ) -> Result<(String, String), RunnerError> {
        let use_escalation_first = self.complexity_routing_enabled
            && complexity == ComplexityLevel::High
            && self.escalation_backend.is_some();

        if use_escalation_first {
            // High complexity: try escalation backend first, fall back to primary.
            self.call_escalation_then_primary(agent_id, prompt).await
        } else {
            // Low/Medium complexity (or routing disabled): primary first.
            self.call_primary_then_escalation(agent_id, prompt).await
        }
    }

    /// Try primary backend first, then escalation backend as fallback.
    ///
    /// Returns the raw response text and the name of the backend that responded.
    async fn call_primary_then_escalation(
        &self,
        agent_id: AgentId,
        prompt: &crate::prompt::RenderedPrompt,
    ) -> Result<(String, String), RunnerError> {
        match self.primary_backend.complete(prompt).await {
            Ok(response) => {
                let name = self.primary_backend.name().to_owned();
                debug!(
                    agent_id = %agent_id,
                    backend = name,
                    response_len = response.len(),
                    "primary backend responded"
                );
                Ok((response, name))
            }
            Err(primary_err) => {
                warn!(
                    agent_id = %agent_id,
                    backend = self.primary_backend.name(),
                    error = %primary_err,
                    "primary backend failed, trying escalation fallback"
                );
                self.try_escalation_fallback(agent_id, prompt).await
            }
        }
    }

    /// Try escalation backend first, then primary backend as fallback.
    ///
    /// Returns the raw response text and the name of the backend that responded.
    async fn call_escalation_then_primary(
        &self,
        agent_id: AgentId,
        prompt: &crate::prompt::RenderedPrompt,
    ) -> Result<(String, String), RunnerError> {
        if let Some(escalation) = &self.escalation_backend {
            match escalation.complete(prompt).await {
                Ok(response) => {
                    let name = escalation.name().to_owned();
                    info!(
                        agent_id = %agent_id,
                        backend = name,
                        response_len = response.len(),
                        "escalation backend responded (high complexity)"
                    );
                    return Ok((response, name));
                }
                Err(escalation_err) => {
                    warn!(
                        agent_id = %agent_id,
                        backend = escalation.name(),
                        error = %escalation_err,
                        "escalation backend failed, falling back to primary"
                    );
                }
            }
        }

        // Fall back to primary
        match self.primary_backend.complete(prompt).await {
            Ok(response) => {
                let name = self.primary_backend.name().to_owned();
                debug!(
                    agent_id = %agent_id,
                    backend = name,
                    response_len = response.len(),
                    "primary backend responded (escalation fallback)"
                );
                Ok((response, name))
            }
            Err(primary_err) => {
                warn!(
                    agent_id = %agent_id,
                    backend = self.primary_backend.name(),
                    error = %primary_err,
                    "both backends failed"
                );
                Err(primary_err)
            }
        }
    }

    /// Try the escalation backend as a fallback (after primary failure).
    ///
    /// Returns the raw response text and the name of the backend that responded.
    async fn try_escalation_fallback(
        &self,
        agent_id: AgentId,
        prompt: &crate::prompt::RenderedPrompt,
    ) -> Result<(String, String), RunnerError> {
        if let Some(escalation) = &self.escalation_backend {
            match escalation.complete(prompt).await {
                Ok(response) => {
                    let name = escalation.name().to_owned();
                    info!(
                        agent_id = %agent_id,
                        backend = name,
                        "escalation backend responded after primary failure"
                    );
                    Ok((response, name))
                }
                Err(escalation_err) => {
                    warn!(
                        agent_id = %agent_id,
                        backend = escalation.name(),
                        error = %escalation_err,
                        "escalation backend also failed"
                    );
                    Err(escalation_err)
                }
            }
        } else {
            warn!(
                agent_id = %agent_id,
                "no escalation backend configured"
            );
            Err(RunnerError::LlmBackend(
                "primary failed and no escalation backend configured".to_owned(),
            ))
        }
    }

    /// Build and publish a [`DecisionRecord`] for an action using a typed
    /// [`DecisionSource`].
    fn publish_decision_record(
        &self,
        action: &ActionRequest,
        source: DecisionSource,
        llm_meta: Option<&LlmDecisionMeta>,
        rule_matched: Option<&str>,
    ) {
        self.publish_decision_record_with_source(
            action,
            source.as_str(),
            llm_meta,
            rule_matched,
        );
    }

    /// Build and publish a [`DecisionRecord`] for an action using a raw
    /// decision source string (used for `"timeout"` which has no
    /// [`DecisionSource`] variant).
    fn publish_decision_record_with_source(
        &self,
        action: &ActionRequest,
        source: &str,
        llm_meta: Option<&LlmDecisionMeta>,
        rule_matched: Option<&str>,
    ) {
        let action_params = serde_json::to_value(&action.parameters).unwrap_or_default();

        let record = DecisionRecord {
            agent_id: action.agent_id,
            tick: action.tick,
            decision_source: source.to_owned(),
            action_type: format!("{:?}", action.action_type),
            action_params,
            llm_backend: llm_meta.map(|m| m.backend_name.clone()),
            model: None, // Model ID is not directly available from the backend
            prompt_tokens: None,
            completion_tokens: None,
            cost_usd: None,
            latency_ms: llm_meta.map(|m| m.latency_ms),
            raw_llm_response: llm_meta.map(|m| m.raw_response.clone()),
            prompt_sent: llm_meta.map(|m| m.prompt_sent.clone()),
            rule_matched: rule_matched.map(ToOwned::to_owned),
            created_at: Utc::now(),
        };

        self.nats.publish_decision(&record);
    }
}

/// Truncate a string to at most `max_len` bytes on a valid UTF-8 boundary.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else {
        // Find the last valid char boundary at or before max_len.
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        let mut truncated = s[..end].to_owned();
        truncated.push_str("...");
        truncated
    }
}

/// Construct a `NoAction` request for an agent that could not decide.
fn no_action_request(agent_id: AgentId, tick: u64) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::NoAction,
        parameters: ActionParameters::NoAction,
        submitted_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::RenderedPrompt;

    fn test_perception() -> Perception {
        serde_json::from_value(serde_json::json!({
            "tick": 10,
            "time_of_day": "Morning",
            "season": "Summer",
            "weather": "Clear",
            "self_state": {
                "id": "01945c2a-3b4f-7def-8a12-bc34567890ab",
                "name": "TestAgent",
                "age": 5,
                "energy": 80,
                "health": 100,
                "hunger": 10,
                "thirst": 0,
                "location_name": "Forest",
                "inventory": {},
                "carry_load": "0/50",
                "active_goals": ["find food"],
                "known_skills": []
            },
            "surroundings": {
                "location_description": "A dense forest",
                "visible_resources": {"Wood": "abundant"},
                "structures_here": [],
                "agents_here": [],
                "messages_here": []
            },
            "known_routes": [],
            "recent_memory": [],
            "available_actions": ["gather", "rest", "move"],
            "notifications": []
        })).unwrap_or_else(|_| {
            Perception {
                tick: 10,
                time_of_day: emergence_types::TimeOfDay::Morning,
                season: emergence_types::Season::Summer,
                weather: emergence_types::Weather::Clear,
                self_state: emergence_types::SelfState {
                    id: emergence_types::AgentId::new(),
                    name: "TestAgent".to_owned(),
                    sex: emergence_types::Sex::Male,
                    age: 5,
                    energy: 80,
                    health: 100,
                    hunger: 10,
                    thirst: 0,
                    location_name: "Forest".to_owned(),
                    inventory: std::collections::BTreeMap::new(),
                    carry_load: "0/50".to_owned(),
                    active_goals: vec!["find food".to_owned()],
                    known_skills: Vec::new(),
                },
                surroundings: emergence_types::Surroundings {
                    location_description: "A dense forest".to_owned(),
                    visible_resources: std::collections::BTreeMap::new(),
                    structures_here: Vec::new(),
                    agents_here: Vec::new(),
                    messages_here: Vec::new(),
                },
                known_routes: Vec::new(),
                recent_memory: Vec::new(),
                available_actions: vec!["gather".to_owned(), "rest".to_owned(), "move".to_owned()],
                notifications: Vec::new(),
            }
        })
    }

    fn test_prompt_engine() -> PromptEngine {
        // Use a unique directory per thread to avoid race conditions when tests
        // run in parallel.
        let unique = format!(
            "emergence_runner_decide_templates_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join("system.j2"), "You are an agent.").ok();
        std::fs::write(dir.join("identity.j2"), "Name: {{ self_state.name }}").ok();
        std::fs::write(dir.join("perception.j2"), "Tick: {{ tick }}").ok();
        std::fs::write(dir.join("memory.j2"), "No memories.").ok();
        std::fs::write(
            dir.join("actions.j2"),
            "Available: {% for a in available_actions %}{{ a }} {% endfor %}\nRespond with JSON.",
        )
        .ok();
        PromptEngine::new(dir.to_str().unwrap_or("")).unwrap_or_else(|e| {
            tracing::error!("failed to create prompt engine: {e}");
            std::process::exit(1);
        })
    }

    #[tokio::test]
    async fn decide_with_mock_llm_response() {
        let prompt_engine = test_prompt_engine();
        let perception = test_perception();
        let agent_id = perception.self_state.id;

        // Simulate what the runner does: render prompt, get response, parse
        let perception_json = serde_json::to_value(&perception).unwrap_or_default();
        let prompt = prompt_engine.render(&perception_json).unwrap_or_else(|_| {
            RenderedPrompt {
                system: String::new(),
                user: String::new(),
            }
        });

        assert!(!prompt.system.is_empty(), "system prompt should be non-empty");
        assert!(!prompt.user.is_empty(), "user prompt should be non-empty");

        // Mock LLM response
        let raw_response = r#"{"action_type": "Gather", "parameters": {"resource": "Wood"}, "reasoning": "Need wood for shelter"}"#;
        let decision = parse_llm_response(raw_response);
        assert_eq!(decision.action_type, ActionType::Gather);

        let action = ActionRequest {
            agent_id,
            tick: 10,
            action_type: decision.action_type,
            parameters: decision.parameters,
            submitted_at: Utc::now(),
        };
        assert_eq!(action.action_type, ActionType::Gather);
        assert_eq!(action.tick, 10);
    }

    #[tokio::test]
    async fn timeout_produces_noaction() {
        let agent_id = AgentId::new();
        let timeout_result = timeout(
            Duration::from_millis(10),
            tokio::time::sleep(Duration::from_secs(60)),
        )
        .await;

        assert!(timeout_result.is_err(), "should have timed out");

        // On timeout, runner submits NoAction
        let action = no_action_request(agent_id, 1);
        assert_eq!(action.action_type, ActionType::NoAction);
        assert!(matches!(action.parameters, ActionParameters::NoAction));
    }

    #[tokio::test]
    async fn fallback_chain_simulation() {
        let prompt_engine = test_prompt_engine();
        let perception = test_perception();

        let perception_json = serde_json::to_value(&perception).unwrap_or_default();
        let _prompt = prompt_engine.render(&perception_json).unwrap_or_else(|_| {
            RenderedPrompt {
                system: String::new(),
                user: String::new(),
            }
        });

        // Simulate: primary fails
        let primary_result: Result<String, RunnerError> =
            Err(RunnerError::LlmBackend("primary is down".to_owned()));
        assert!(primary_result.is_err());

        // Simulate: secondary succeeds
        let secondary_response =
            r#"{"action_type": "Rest", "parameters": {}, "reasoning": "Primary was down"}"#;
        let decision = parse_llm_response(secondary_response);
        assert_eq!(decision.action_type, ActionType::Rest);

        // Simulate: both fail -> NoAction
        let both_failed: Result<String, RunnerError> =
            Err(RunnerError::LlmBackend("secondary also down".to_owned()));
        assert!(both_failed.is_err());
        let fallback_action = no_action_request(perception.self_state.id, 10);
        assert_eq!(fallback_action.action_type, ActionType::NoAction);
    }

    #[test]
    fn no_action_request_is_valid() {
        let agent_id = AgentId::new();
        let action = no_action_request(agent_id, 42);
        assert_eq!(action.agent_id, agent_id);
        assert_eq!(action.tick, 42);
        assert_eq!(action.action_type, ActionType::NoAction);
        assert!(matches!(action.parameters, ActionParameters::NoAction));
    }

    #[test]
    fn complexity_scoring_solo_perception() {
        let perception = test_perception();
        let complexity = score_complexity(&perception);
        // Solo survival perception with no agents, no messages, clear weather
        // should be Low complexity.
        assert_eq!(complexity, ComplexityLevel::Low);
    }

    #[test]
    fn complexity_scoring_social_perception() {
        let mut perception = test_perception();
        // Add agents and social actions to push into Medium.
        perception.surroundings.agents_here = vec![
            emergence_types::VisibleAgent {
                name: "Neighbor".to_owned(),
                sex: emergence_types::Sex::Male,
                relationship: "friendly (0.5)".to_owned(),
                activity: "idle".to_owned(),
            },
        ];
        perception.available_actions.push("communicate".to_owned());

        let complexity = score_complexity(&perception);
        // 1 (agent) + 2 (social actions) = 3 => Medium.
        assert_eq!(complexity, ComplexityLevel::Medium);
    }
}
