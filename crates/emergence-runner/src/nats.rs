//! NATS pub/sub integration for perception intake and action submission.
//!
//! The World Engine publishes perception payloads on subjects matching
//! `tick.{N}.perception.{agent_id}`. The runner subscribes to all perception
//! subjects, processes each one through the LLM pipeline, and publishes
//! the resulting action on `tick.{N}.action.{agent_id}`.

use emergence_types::{ActionRequest, DecisionRecord, Perception};
use tracing::{debug, info, warn};

use crate::error::RunnerError;

/// NATS client wrapper for the agent runner.
///
/// Manages a single NATS connection and provides methods for subscribing
/// to perception deliveries and publishing action submissions.
pub struct NatsClient {
    client: async_nats::Client,
}

impl NatsClient {
    /// Connect to a NATS server.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::Nats`] if the connection cannot be established.
    pub async fn connect(url: &str) -> Result<Self, RunnerError> {
        info!(url = url, "connecting to NATS server");
        let client = async_nats::connect(url)
            .await
            .map_err(|e| RunnerError::Nats(format!("failed to connect to {url}: {e}")))?;
        info!("NATS connection established");
        Ok(Self { client })
    }

    /// Subscribe to all perception subjects.
    ///
    /// Returns a subscription that yields messages matching
    /// `tick.*.perception.*` (all agents, all ticks).
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::Nats`] if the subscription fails.
    pub async fn subscribe_perceptions(
        &self,
    ) -> Result<async_nats::Subscriber, RunnerError> {
        let subject = "tick.*.perception.*";
        debug!(subject = subject, "subscribing to perception subjects");
        let subscriber = self
            .client
            .subscribe(subject.to_owned())
            .await
            .map_err(|e| {
                RunnerError::Nats(format!("failed to subscribe to {subject}: {e}"))
            })?;
        info!("subscribed to perception subjects");
        Ok(subscriber)
    }

    /// Check whether a given agent ID belongs to this runner's partition.
    ///
    /// Uses the first 8 bytes of the UUID (which is time-ordered in `UUIDv7`)
    /// hashed modulo `total_partitions`. When `total_partitions == 1`, all
    /// agents belong to partition 0 (single-runner mode).
    ///
    /// Returns `true` if the agent should be handled by this runner instance.
    pub fn is_my_agent(
        agent_id: &emergence_types::AgentId,
        partition_id: u32,
        total_partitions: u32,
    ) -> bool {
        if total_partitions <= 1 {
            return true;
        }
        // Use the UUID bytes directly for a deterministic partition assignment.
        // We take bytes 0..4 as a big-endian u32 for a fast, stable hash.
        let uuid = agent_id.into_inner();
        let bytes = uuid.as_bytes();
        let hash = u32::from_be_bytes([
            *bytes.first().unwrap_or(&0),
            *bytes.get(1).unwrap_or(&0),
            *bytes.get(2).unwrap_or(&0),
            *bytes.get(3).unwrap_or(&0),
        ]);
        #[allow(clippy::arithmetic_side_effects)]
        {
            hash.wrapping_rem(total_partitions) == partition_id
        }
    }

    /// Publish an action response for a specific agent and tick.
    ///
    /// The subject is `tick.{tick}.action.{agent_id}`.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::Nats`] if serialization or publishing fails.
    pub async fn publish_action(
        &self,
        tick: u64,
        action: &ActionRequest,
    ) -> Result<(), RunnerError> {
        let subject = format!("tick.{tick}.action.{}", action.agent_id);
        let payload = serde_json::to_vec(action)
            .map_err(|e| RunnerError::Nats(format!("failed to serialize action: {e}")))?;
        debug!(
            subject = subject,
            agent_id = %action.agent_id,
            action_type = ?action.action_type,
            "publishing action"
        );
        self.client
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| RunnerError::Nats(format!("failed to publish to {subject}: {e}")))?;
        Ok(())
    }

    /// Publish a decision record for observability (fire-and-forget).
    ///
    /// The subject is `emergence.decisions.<tick>.<agent_id>`.
    /// Serialization or publish failures are logged but do not propagate --
    /// decision logging must never block the decision pipeline.
    pub fn publish_decision(&self, record: &DecisionRecord) {
        let subject = format!(
            "emergence.decisions.{}.{}",
            record.tick, record.agent_id
        );
        match serde_json::to_vec(record) {
            Ok(payload) => {
                let client = self.client.clone();
                tokio::spawn(async move {
                    if let Err(e) = client.publish(subject.clone(), payload.into()).await {
                        warn!(
                            subject = subject,
                            error = %e,
                            "failed to publish decision record"
                        );
                    }
                });
            }
            Err(e) => {
                warn!(
                    subject = subject,
                    error = %e,
                    "failed to serialize decision record"
                );
            }
        }
    }

    /// Deserialize a NATS message payload into a [`Perception`].
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::Parse`] if deserialization fails.
    pub fn deserialize_perception(data: &[u8]) -> Result<Perception, RunnerError> {
        serde_json::from_slice(data)
            .map_err(|e| RunnerError::Parse(format!("failed to deserialize perception: {e}")))
    }

    /// Extract the tick number from a perception subject string.
    ///
    /// Subject format: `tick.{N}.perception.{agent_id}`
    ///
    /// Returns `None` if the subject does not match the expected format.
    pub fn extract_tick_from_subject(subject: &str) -> Option<u64> {
        let parts: Vec<&str> = subject.split('.').collect();
        if parts.len() >= 4 {
            parts.get(1).and_then(|s| s.parse().ok())
        } else {
            None
        }
    }

    /// Flush all pending messages to the NATS server.
    ///
    /// Currently unused but will be needed when batching action
    /// submissions in multi-agent parallel processing.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::Nats`] if the flush operation fails.
    #[allow(dead_code)]
    pub async fn flush(&self) -> Result<(), RunnerError> {
        self.client
            .flush()
            .await
            .map_err(|e| RunnerError::Nats(format!("flush failed: {e}")))
    }
}

impl std::fmt::Debug for NatsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsClient")
            .field("connected", &true)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tick_from_valid_subject() {
        let tick = NatsClient::extract_tick_from_subject("tick.42.perception.some-agent-id");
        assert_eq!(tick, Some(42));
    }

    #[test]
    fn extract_tick_from_large_number() {
        let tick = NatsClient::extract_tick_from_subject("tick.999999.perception.abc");
        assert_eq!(tick, Some(999_999));
    }

    #[test]
    fn extract_tick_from_invalid_subject() {
        let tick = NatsClient::extract_tick_from_subject("invalid.subject");
        assert_eq!(tick, None);
    }

    #[test]
    fn extract_tick_from_non_numeric() {
        let tick = NatsClient::extract_tick_from_subject("tick.abc.perception.xyz");
        assert_eq!(tick, None);
    }

    #[test]
    fn deserialize_valid_perception() {
        let perception_json = serde_json::json!({
            "tick": 1,
            "time_of_day": "Morning",
            "season": "Summer",
            "weather": "Clear",
            "self_state": {
                "id": "01945c2a-3b4f-7def-8a12-bc34567890ab",
                "name": "TestAgent",
                "sex": "Male",
                "age": 5,
                "energy": 80,
                "health": 100,
                "hunger": 10,
                "thirst": 0,
                "location_name": "Forest Clearing",
                "inventory": {},
                "carry_load": "0/50",
                "active_goals": [],
                "known_skills": []
            },
            "surroundings": {
                "location_description": "A peaceful clearing",
                "visible_resources": {},
                "structures_here": [],
                "agents_here": [],
                "messages_here": []
            },
            "known_routes": [],
            "recent_memory": [],
            "available_actions": ["gather", "rest"],
            "notifications": []
        });

        let bytes = serde_json::to_vec(&perception_json).unwrap_or_default();
        let result = NatsClient::deserialize_perception(&bytes);
        assert!(result.is_ok());
        let perception = result.unwrap_or_else(|_| {
            // This branch should not be reached; provide a dummy for type safety.
            serde_json::from_value(perception_json).unwrap_or_else(|_| {
                Perception {
                    tick: 0,
                    time_of_day: emergence_types::TimeOfDay::Morning,
                    season: emergence_types::Season::Summer,
                    weather: emergence_types::Weather::Clear,
                    self_state: emergence_types::SelfState {
                        id: emergence_types::AgentId::new(),
                        name: String::new(),
                        sex: emergence_types::Sex::Male,
                        age: 0,
                        energy: 0,
                        health: 0,
                        hunger: 0,
                        thirst: 0,
                        location_name: String::new(),
                        inventory: std::collections::BTreeMap::new(),
                        carry_load: String::new(),
                        active_goals: Vec::new(),
                        known_skills: Vec::new(),
                    },
                    surroundings: emergence_types::Surroundings {
                        location_description: String::new(),
                        visible_resources: std::collections::BTreeMap::new(),
                        structures_here: Vec::new(),
                        agents_here: Vec::new(),
                        messages_here: Vec::new(),
                    },
                    known_routes: Vec::new(),
                    recent_memory: Vec::new(),
                    available_actions: Vec::new(),
                    notifications: Vec::new(),
                    personality: None,
                }
            })
        });
        assert_eq!(perception.tick, 1);
        assert_eq!(perception.self_state.name, "TestAgent");
    }

    #[test]
    fn deserialize_invalid_perception() {
        let result = NatsClient::deserialize_perception(b"not valid json");
        assert!(result.is_err());
    }

    // Integration tests that require a live NATS server are marked #[ignore].
    #[tokio::test]
    #[ignore]
    async fn connect_to_nats() {
        let result = NatsClient::connect("nats://localhost:4222").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn subscribe_and_publish() {
        let client = NatsClient::connect("nats://localhost:4222")
            .await
            .unwrap_or_else(|e| {
                tracing::error!("NATS connection failed: {e}");
                std::process::exit(1);
            });

        let _subscriber = client
            .subscribe_perceptions()
            .await
            .unwrap_or_else(|e| {
                tracing::error!("subscription failed: {e}");
                std::process::exit(1);
            });
    }

    #[test]
    fn single_partition_accepts_all_agents() {
        let agent_id = emergence_types::AgentId::new();
        assert!(NatsClient::is_my_agent(&agent_id, 0, 1));
    }

    #[test]
    fn partitioning_is_deterministic() {
        let agent_id = emergence_types::AgentId::new();
        // Same agent always maps to the same partition.
        let result_a = NatsClient::is_my_agent(&agent_id, 0, 4);
        let result_b = NatsClient::is_my_agent(&agent_id, 0, 4);
        assert_eq!(result_a, result_b);
    }

    #[test]
    fn partitioning_covers_all_agents() {
        // With 4 partitions, every agent belongs to exactly one.
        let total = 4;
        for _ in 0..20 {
            let agent_id = emergence_types::AgentId::new();
            let mut count = 0u32;
            for pid in 0..total {
                if NatsClient::is_my_agent(&agent_id, pid, total) {
                    count = count.saturating_add(1);
                }
            }
            assert_eq!(count, 1, "agent should belong to exactly one partition");
        }
    }
}
