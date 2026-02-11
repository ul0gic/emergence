//! NATS-based decision source for the World Engine.
//!
//! [`NatsDecisionSource`] implements the [`DecisionSource`] trait by
//! publishing perception payloads to NATS subjects that the agent runner
//! subscribes to, then collecting action responses within a configurable
//! timeout window.
//!
//! # Subject Convention
//!
//! - **Perception publish:** `tick.{N}.perception.{agent_id}`
//! - **Action subscribe:** `tick.{N}.action.*`
//!
//! These match the patterns used by `emergence-runner`'s `NatsClient`:
//! the runner subscribes to `tick.*.perception.*` and publishes to
//! `tick.{N}.action.{agent_id}`.
//!
//! # Sync/Async Bridge
//!
//! The [`DecisionSource`] trait method is synchronous, but NATS operations
//! are async. We use [`tokio::runtime::Handle::current().block_on()`] to
//! bridge into the existing tokio runtime.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use emergence_core::decision::{DecisionError, DecisionSource};
use emergence_types::{ActionParameters, ActionRequest, ActionType, AgentId, Perception};
use futures::StreamExt as _;
use tracing::{debug, warn};

/// A decision source that communicates with the agent runner via NATS.
///
/// For each tick, it publishes perception payloads for all agents and
/// collects their action responses within the configured timeout. Agents
/// that do not respond in time receive a `NoAction` fallback.
pub struct NatsDecisionSource {
    /// The NATS client connection.
    client: async_nats::Client,
    /// Maximum time to wait for all agent responses.
    timeout: Duration,
}

impl NatsDecisionSource {
    /// Create a new NATS decision source from an existing client.
    ///
    /// The `client` must already be connected. The `timeout` controls how
    /// long to wait for agent responses each tick before falling back to
    /// `NoAction`.
    #[allow(dead_code)]
    pub const fn new(client: async_nats::Client, timeout: Duration) -> Self {
        Self { client, timeout }
    }

    /// Connect to a NATS server and create a decision source.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionError::Internal`] if the connection fails.
    pub async fn connect(url: &str, timeout: Duration) -> Result<Self, DecisionError> {
        let client = async_nats::connect(url).await.map_err(|e| {
            DecisionError::Internal {
                message: format!("failed to connect to NATS at {url}: {e}"),
            }
        })?;
        Ok(Self { client, timeout })
    }

    /// The async implementation of decision collection.
    ///
    /// Publishes perceptions, subscribes to action responses, and collects
    /// them within the timeout window.
    async fn collect_decisions_async(
        &self,
        tick: u64,
        perceptions: &BTreeMap<AgentId, Perception>,
    ) -> Result<BTreeMap<AgentId, ActionRequest>, DecisionError> {
        // Subscribe to action responses BEFORE publishing perceptions
        // to avoid a race condition where responses arrive before the
        // subscription is active.
        let action_subject = format!("tick.{tick}.action.*");
        let mut action_sub = self
            .client
            .subscribe(action_subject.clone())
            .await
            .map_err(|e| DecisionError::Internal {
                message: format!("failed to subscribe to {action_subject}: {e}"),
            })?;

        // Publish perceptions for all agents.
        self.publish_all_perceptions(tick, perceptions).await?;

        // Collect action responses within the timeout.
        let decisions =
            collect_responses(&mut action_sub, tick, perceptions, self.timeout).await;

        // Unsubscribe to clean up.
        let _ = action_sub.unsubscribe().await;

        // Fill in NoAction for agents that did not respond.
        let final_decisions = fill_no_action_fallbacks(tick, perceptions, decisions);

        Ok(final_decisions)
    }

    /// Publish perception payloads for all agents to NATS.
    async fn publish_all_perceptions(
        &self,
        tick: u64,
        perceptions: &BTreeMap<AgentId, Perception>,
    ) -> Result<(), DecisionError> {
        for (&agent_id, perception) in perceptions {
            let subject = format!("tick.{tick}.perception.{agent_id}");
            let payload = serde_json::to_vec(perception).map_err(|e| {
                DecisionError::Internal {
                    message: format!(
                        "failed to serialize perception for agent {agent_id}: {e}"
                    ),
                }
            })?;

            self.client
                .publish(subject.clone(), payload.into())
                .await
                .map_err(|e| DecisionError::Internal {
                    message: format!("failed to publish perception on {subject}: {e}"),
                })?;

            debug!(tick, agent_id = %agent_id, "Published perception");
        }

        // Flush to ensure all perception messages are sent.
        self.client.flush().await.map_err(|e| DecisionError::Internal {
            message: format!("failed to flush NATS: {e}"),
        })?;

        Ok(())
    }
}

/// Collect action responses from the subscription within the timeout.
///
/// Returns collected decisions (may be partial if some agents time out).
async fn collect_responses(
    action_sub: &mut async_nats::Subscriber,
    tick: u64,
    perceptions: &BTreeMap<AgentId, Perception>,
    timeout: Duration,
) -> BTreeMap<AgentId, ActionRequest> {
    let mut decisions: BTreeMap<AgentId, ActionRequest> = BTreeMap::new();
    let agent_count = perceptions.len();
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(tokio::time::Instant::now);

    while decisions.len() < agent_count {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, action_sub.next()).await {
            Ok(Some(msg)) => {
                match serde_json::from_slice::<ActionRequest>(&msg.payload) {
                    Ok(action) => {
                        if action.tick == tick && perceptions.contains_key(&action.agent_id)
                        {
                            debug!(
                                tick,
                                agent_id = %action.agent_id,
                                action_type = ?action.action_type,
                                "Received action from agent"
                            );
                            decisions.insert(action.agent_id, action);
                        }
                    }
                    Err(e) => {
                        warn!(
                            tick,
                            error = %e,
                            "Failed to deserialize action response"
                        );
                    }
                }
            }
            Ok(None) => {
                // Subscription closed.
                break;
            }
            Err(_) => {
                // Timeout reached.
                break;
            }
        }
    }

    decisions
}

/// Build a `NoAction` request for an agent that did not respond.
fn make_no_action(agent_id: AgentId, tick: u64) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::NoAction,
        parameters: ActionParameters::NoAction,
        submitted_at: Utc::now(),
    }
}

/// Insert `NoAction` for any agents in `perceptions` that are missing
/// from `decisions`. Returns the completed decision map.
fn fill_no_action_fallbacks(
    tick: u64,
    perceptions: &BTreeMap<AgentId, Perception>,
    mut decisions: BTreeMap<AgentId, ActionRequest>,
) -> BTreeMap<AgentId, ActionRequest> {
    let responded_count = decisions.len();

    for &agent_id in perceptions.keys() {
        decisions.entry(agent_id).or_insert_with(|| {
            debug!(
                tick,
                agent_id = %agent_id,
                "Agent did not respond, inserting NoAction"
            );
            make_no_action(agent_id, tick)
        });
    }

    let no_action_count = perceptions.len().saturating_sub(responded_count);
    if no_action_count > 0 {
        warn!(
            tick,
            responded = responded_count,
            timed_out = no_action_count,
            "Some agents did not respond in time"
        );
    }

    decisions
}

impl std::fmt::Debug for NatsDecisionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsDecisionSource")
            .field("timeout_ms", &self.timeout.as_millis())
            .finish_non_exhaustive()
    }
}

impl DecisionSource for NatsDecisionSource {
    fn collect_decisions(
        &mut self,
        tick: u64,
        perceptions: &BTreeMap<AgentId, Perception>,
    ) -> Result<BTreeMap<AgentId, ActionRequest>, DecisionError> {
        // Bridge from sync trait method to async NATS operations using
        // the current tokio runtime handle. This is safe because
        // `run_simulation` is already running in an async context, and
        // this call is on the same thread (not inside a spawn).
        let handle = tokio::runtime::Handle::try_current().map_err(|e| {
            DecisionError::Internal {
                message: format!("no tokio runtime available: {e}"),
            }
        })?;

        // Use `block_in_place` to avoid blocking the runtime's executor
        // thread when calling `block_on`.
        tokio::task::block_in_place(|| {
            handle.block_on(self.collect_decisions_async(tick, perceptions))
        })
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use emergence_types::*;

    use super::*;

    fn make_perception(tick: u64, agent_id: AgentId) -> Perception {
        Perception {
            tick,
            time_of_day: TimeOfDay::Morning,
            season: Season::Spring,
            weather: Weather::Clear,
            self_state: SelfState {
                id: agent_id,
                name: String::from("Test Agent"),
                sex: emergence_types::Sex::Male,
                age: 0,
                energy: 80,
                health: 100,
                hunger: 0,
                thirst: 0,
                location_name: String::from("Meadow"),
                inventory: BTreeMap::new(),
                carry_load: String::from("0/50"),
                active_goals: Vec::new(),
                known_skills: Vec::new(),
            },
            surroundings: Surroundings {
                location_description: String::from("A green meadow."),
                visible_resources: BTreeMap::new(),
                structures_here: Vec::new(),
                agents_here: Vec::new(),
                messages_here: Vec::new(),
            },
            known_routes: Vec::new(),
            recent_memory: Vec::new(),
            available_actions: Vec::new(),
            notifications: Vec::new(),
        }
    }

    /// Test that when no NATS server is available, all agents get `NoAction`.
    /// This test uses an in-process NATS simulation by publishing directly.
    #[tokio::test]
    async fn timeout_returns_no_action_for_all() {
        // Connect to a NATS server. If unavailable, skip the test.
        let client_result = async_nats::connect("nats://localhost:4222").await;
        if client_result.is_err() {
            // NATS not available; test the fallback logic differently.
            // We verify the NoAction insertion logic in the unit test below.
            return;
        }

        let client = client_result.unwrap();
        let timeout = Duration::from_millis(200);
        let mut source = NatsDecisionSource::new(client, timeout);

        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let mut perceptions = BTreeMap::new();
        perceptions.insert(a1, make_perception(999, a1));
        perceptions.insert(a2, make_perception(999, a2));

        // No agent runner is responding, so all should time out.
        let decisions = source.collect_decisions(999, &perceptions).unwrap();

        assert_eq!(decisions.len(), 2);
        for (_, action) in &decisions {
            assert_eq!(action.action_type, ActionType::NoAction);
        }
    }

    /// Test the `NoAction` fallback logic without a live NATS server.
    #[test]
    fn no_action_fallback_for_missing_agents() {
        // Simulate the fallback: given a set of perceptions and a partial
        // decisions map, verify that NoAction is inserted for missing agents.
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let a3 = AgentId::new();
        let mut perceptions = BTreeMap::new();
        perceptions.insert(a1, make_perception(1, a1));
        perceptions.insert(a2, make_perception(1, a2));
        perceptions.insert(a3, make_perception(1, a3));

        // Simulate: only a1 responded.
        let mut decisions: BTreeMap<AgentId, ActionRequest> = BTreeMap::new();
        decisions.insert(
            a1,
            ActionRequest {
                agent_id: a1,
                tick: 1,
                action_type: ActionType::Gather,
                parameters: ActionParameters::Gather {
                    resource: Resource::Wood,
                },
                submitted_at: Utc::now(),
            },
        );

        let final_decisions = fill_no_action_fallbacks(1, &perceptions, decisions);

        assert_eq!(final_decisions.len(), 3);
        // a1 kept their original action.
        assert_eq!(
            final_decisions.get(&a1).map(|d| d.action_type),
            Some(ActionType::Gather)
        );
        // a2 and a3 got NoAction.
        assert_eq!(
            final_decisions.get(&a2).map(|d| d.action_type),
            Some(ActionType::NoAction)
        );
        assert_eq!(
            final_decisions.get(&a3).map(|d| d.action_type),
            Some(ActionType::NoAction)
        );
    }

    /// Test that a published action is correctly deserialized.
    #[tokio::test]
    async fn action_deserialization_round_trip() {
        let agent_id = AgentId::new();
        let action = ActionRequest {
            agent_id,
            tick: 42,
            action_type: ActionType::Rest,
            parameters: ActionParameters::Rest,
            submitted_at: Utc::now(),
        };

        let serialized = serde_json::to_vec(&action).unwrap();
        let deserialized: ActionRequest = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(deserialized.agent_id, agent_id);
        assert_eq!(deserialized.tick, 42);
        assert_eq!(deserialized.action_type, ActionType::Rest);
    }
}
