//! Decision source trait and stub implementation.
//!
//! During the Decision phase of the tick cycle, the engine presents each
//! agent with a [`Perception`] payload and awaits an [`ActionRequest`] in
//! response. The [`DecisionSource`] trait abstracts the mechanism by which
//! decisions are obtained -- it could be an LLM backend, a scripted bot,
//! a human player, or a test stub.
//!
//! For Phase 2, the [`StubDecisionSource`] always returns
//! [`ActionType::NoAction`], which allows the tick cycle to be exercised
//! end-to-end before the LLM agent runner is implemented.

use std::collections::BTreeMap;

use chrono::Utc;
use emergence_types::{ActionParameters, ActionRequest, ActionType, AgentId, Perception};

/// Errors that can occur during the decision phase.
#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    /// The agent did not respond within the deadline.
    #[error("agent {agent_id} timed out (deadline: {deadline_ms}ms)")]
    Timeout {
        /// The agent that timed out.
        agent_id: AgentId,
        /// The deadline in milliseconds.
        deadline_ms: u64,
    },

    /// An internal error in the decision source.
    #[error("decision source error: {message}")]
    Internal {
        /// Description of the error.
        message: String,
    },
}

/// A source of agent decisions.
///
/// Implementations of this trait produce [`ActionRequest`] values for agents
/// when given their [`Perception`] payloads. The engine calls
/// [`collect_decisions`] once per tick during the Decision phase.
///
/// [`collect_decisions`]: DecisionSource::collect_decisions
pub trait DecisionSource {
    /// Collect decisions from all agents for the given tick.
    ///
    /// `perceptions` maps agent IDs to their perception payloads. The
    /// implementation should return an `ActionRequest` for each agent.
    /// Agents that time out or fail should receive a `NoAction` request.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionError`] if the decision process fails entirely
    /// (individual agent failures should be handled by returning `NoAction`
    /// for that agent).
    fn collect_decisions(
        &mut self,
        tick: u64,
        perceptions: &BTreeMap<AgentId, Perception>,
    ) -> Result<BTreeMap<AgentId, ActionRequest>, DecisionError>;
}

/// A stub decision source that always returns [`ActionType::NoAction`].
///
/// Used in Phase 2 to exercise the tick cycle without an LLM backend.
/// Every agent effectively forfeits their turn each tick.
#[derive(Debug, Clone, Default)]
pub struct StubDecisionSource;

impl StubDecisionSource {
    /// Create a new stub decision source.
    pub const fn new() -> Self {
        Self
    }
}

impl DecisionSource for StubDecisionSource {
    fn collect_decisions(
        &mut self,
        tick: u64,
        perceptions: &BTreeMap<AgentId, Perception>,
    ) -> Result<BTreeMap<AgentId, ActionRequest>, DecisionError> {
        let mut decisions = BTreeMap::new();

        for &agent_id in perceptions.keys() {
            decisions.insert(
                agent_id,
                ActionRequest {
                    agent_id,
                    tick,
                    action_type: ActionType::NoAction,
                    parameters: ActionParameters::NoAction,
                    submitted_at: Utc::now(),
                    goal_updates: Vec::new(),
                },
            );
        }

        Ok(decisions)
    }
}

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
            personality: None,
        }
    }

    #[test]
    fn stub_returns_no_action_for_all_agents() {
        let mut source = StubDecisionSource::new();
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        let mut perceptions = BTreeMap::new();
        perceptions.insert(a1, make_perception(1, a1));
        perceptions.insert(a2, make_perception(1, a2));

        let decisions = source.collect_decisions(1, &perceptions);
        assert!(decisions.is_ok());
        let decisions = decisions.unwrap();

        assert_eq!(decisions.len(), 2);
        assert_eq!(
            decisions.get(&a1).map(|d| d.action_type),
            Some(ActionType::NoAction)
        );
        assert_eq!(
            decisions.get(&a2).map(|d| d.action_type),
            Some(ActionType::NoAction)
        );
    }

    #[test]
    fn stub_with_empty_perceptions() {
        let mut source = StubDecisionSource::new();
        let perceptions = BTreeMap::new();

        let decisions = source.collect_decisions(1, &perceptions);
        assert!(decisions.is_ok());
        assert!(decisions.unwrap().is_empty());
    }

    #[test]
    fn stub_uses_correct_tick() {
        let mut source = StubDecisionSource::new();
        let agent = AgentId::new();

        let mut perceptions = BTreeMap::new();
        perceptions.insert(agent, make_perception(42, agent));

        let decisions = source.collect_decisions(42, &perceptions).unwrap();
        assert_eq!(decisions.get(&agent).map(|d| d.tick), Some(42));
    }
}
