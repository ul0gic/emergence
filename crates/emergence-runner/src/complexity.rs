//! Tick complexity scoring for dynamic LLM backend selection.
//!
//! Not all agent decisions are equal. An agent alone in the wilderness
//! gathering wood needs a cheap fast model. An agent negotiating a trade,
//! resolving conflict, or encountering a discovery event benefits from a
//! more capable model.
//!
//! This module scores the complexity of each agent's decision context
//! from the perception payload and produces a [`ComplexityLevel`] that
//! the runner uses to route the LLM call to the appropriate backend.
//!
//! Scoring factors are documented in `build-plan.md` task 6.2.2.

use emergence_types::Perception;

// ---------------------------------------------------------------------------
// Complexity level
// ---------------------------------------------------------------------------

/// The complexity tier of an agent's decision context for a given tick.
///
/// Determines which LLM backend handles the decision:
/// - [`Low`](ComplexityLevel::Low) and [`Medium`](ComplexityLevel::Medium)
///   route to the primary (cheap/fast) backend.
/// - [`High`](ComplexityLevel::High) routes to the escalation (capable)
///   backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComplexityLevel {
    /// Solo survival, routine gathering, simple movement.
    ///
    /// No other agents present, no pending messages, no notifications,
    /// calm weather.
    Low,
    /// Basic social interaction, trading, teaching.
    ///
    /// Other agents present, pending messages, structures at location,
    /// or moderate survival pressure.
    Medium,
    /// Conflict, discovery events, governance, complex negotiation.
    ///
    /// Multiple agents in contested situations, active trade actions
    /// available, governance actions, important notifications, severe
    /// weather, or high social complexity.
    High,
}

impl std::fmt::Display for ComplexityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => f.write_str("low"),
            Self::Medium => f.write_str("medium"),
            Self::High => f.write_str("high"),
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring thresholds
// ---------------------------------------------------------------------------

/// Score at or above which the decision is classified as [`ComplexityLevel::Medium`].
const MEDIUM_THRESHOLD: u32 = 3;

/// Score at or above which the decision is classified as [`ComplexityLevel::High`].
const HIGH_THRESHOLD: u32 = 7;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Score the complexity of an agent's decision context.
///
/// Examines the perception payload to count indicators of decision
/// difficulty. Returns a [`ComplexityLevel`] that the runner uses to
/// select the appropriate LLM backend.
///
/// # Scoring factors
///
/// Each factor adds points to a running total:
///
/// | Factor | Points |
/// |--------|--------|
/// | Other agents present | 1 per agent (max 3) |
/// | Pending messages | 1 per message (max 2) |
/// | Structures at location | 1 |
/// | Notifications present | 1 per notification (max 3) |
/// | Severe weather (Storm/Snow) | 2 |
/// | Social actions available | 2 |
/// | Governance actions available | 3 |
/// | Trade actions available | 2 |
/// | High hunger (>= 70) | 1 |
/// | Low health (< 30) | 1 |
/// | Multiple recent memories | 1 |
///
/// The total is then compared against thresholds:
/// - `< 3` => [`Low`](ComplexityLevel::Low)
/// - `3..7` => [`Medium`](ComplexityLevel::Medium)
/// - `>= 7` => [`High`](ComplexityLevel::High)
pub fn score_complexity(perception: &Perception) -> ComplexityLevel {
    let score = compute_raw_score(perception);

    if score >= HIGH_THRESHOLD {
        ComplexityLevel::High
    } else if score >= MEDIUM_THRESHOLD {
        ComplexityLevel::Medium
    } else {
        ComplexityLevel::Low
    }
}

/// Compute the raw numeric complexity score from a perception payload.
///
/// Exposed as a separate function so tests can verify exact scores.
fn compute_raw_score(perception: &Perception) -> u32 {
    let mut score: u32 = 0;

    // --- Social context: other agents present ---
    // Each nearby agent increases decision complexity (capped at 3).
    let agent_count = perception.surroundings.agents_here.len();
    let agent_points = std::cmp::min(agent_count, 3);
    // SAFETY: agent_points is at most 3, score starts at 0 and max total
    // across all factors is bounded well below u32::MAX.
    #[allow(clippy::arithmetic_side_effects)]
    {
        score += u32::try_from(agent_points).unwrap_or(3);
    }

    // --- Messages pending at location ---
    let message_count = perception.surroundings.messages_here.len();
    let message_points = std::cmp::min(message_count, 2);
    #[allow(clippy::arithmetic_side_effects)]
    {
        score += u32::try_from(message_points).unwrap_or(2);
    }

    // --- Structures at location (building/crafting context) ---
    if !perception.surroundings.structures_here.is_empty() {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 1;
        }
    }

    // --- Notifications (approaching winter, shelter damage, deaths, etc.) ---
    let notification_count = perception.notifications.len();
    let notification_points = std::cmp::min(notification_count, 3);
    #[allow(clippy::arithmetic_side_effects)]
    {
        score += u32::try_from(notification_points).unwrap_or(3);
    }

    // --- Severe weather increases complexity ---
    if matches!(
        perception.weather,
        emergence_types::Weather::Storm | emergence_types::Weather::Snow
    ) {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 2;
        }
    }

    // --- Available action types signal complexity ---
    score = score_available_actions(score, &perception.available_actions);

    // --- Survival pressure ---
    if perception.self_state.hunger >= 70 {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 1;
        }
    }
    if perception.self_state.health < 30 {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 1;
        }
    }

    // --- Memory richness: many recent memories suggest complex ongoing situation ---
    if perception.recent_memory.len() >= 3 {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 1;
        }
    }

    score
}

/// Add points for social, trade, and governance actions in the available
/// actions list.
///
/// Returns the updated score.
fn score_available_actions(mut score: u32, available_actions: &[String]) -> u32 {
    let has_social = available_actions.iter().any(|a| {
        let lower = a.to_lowercase();
        lower.contains("communicate")
            || lower.contains("broadcast")
            || lower.contains("teach")
            || lower.contains("formgroup")
            || lower.contains("form_group")
            || lower.contains("reproduce")
    });

    let has_trade = available_actions.iter().any(|a| {
        let lower = a.to_lowercase();
        lower.contains("tradeoffer")
            || lower.contains("trade_offer")
            || lower.contains("tradeaccept")
            || lower.contains("trade_accept")
            || lower.contains("tradereject")
            || lower.contains("trade_reject")
    });

    let has_governance = available_actions.iter().any(|a| {
        let lower = a.to_lowercase();
        lower.contains("legislate")
            || lower.contains("enforce")
            || lower.contains("claim")
    });

    if has_social {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 2;
        }
    }

    if has_trade {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 2;
        }
    }

    if has_governance {
        #[allow(clippy::arithmetic_side_effects)]
        {
            score += 3;
        }
    }

    score
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use emergence_types::{
        Perception, Season, SelfState, Surroundings, TimeOfDay, VisibleAgent, Weather,
    };

    use super::*;

    /// Build a minimal solo-survival perception (no agents, no messages,
    /// no notifications, clear weather).
    fn solo_survival_perception() -> Perception {
        Perception {
            tick: 10,
            time_of_day: TimeOfDay::Morning,
            season: Season::Summer,
            weather: Weather::Clear,
            self_state: SelfState {
                id: emergence_types::AgentId::new(),
                name: "Loner".to_owned(),
                age: 5,
                energy: 80,
                health: 100,
                hunger: 10,
                location_name: "Forest".to_owned(),
                inventory: BTreeMap::new(),
                carry_load: "0/50".to_owned(),
                active_goals: vec!["find food".to_owned()],
                known_skills: Vec::new(),
            },
            surroundings: Surroundings {
                location_description: "A dense forest".to_owned(),
                visible_resources: BTreeMap::new(),
                structures_here: Vec::new(),
                agents_here: Vec::new(),
                messages_here: Vec::new(),
            },
            known_routes: Vec::new(),
            recent_memory: Vec::new(),
            available_actions: vec!["gather".to_owned(), "rest".to_owned(), "move".to_owned()],
            notifications: Vec::new(),
        }
    }

    #[test]
    fn solo_survival_is_low_complexity() {
        let perception = solo_survival_perception();
        let level = score_complexity(&perception);
        assert_eq!(level, ComplexityLevel::Low);
        // Raw score should be 0 -- no complexity indicators.
        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 0);
    }

    #[test]
    fn agents_present_increases_complexity() {
        let mut perception = solo_survival_perception();
        perception.surroundings.agents_here = vec![
            VisibleAgent {
                name: "Neighbor".to_owned(),
                relationship: "neutral (0.0)".to_owned(),
                activity: "resting".to_owned(),
            },
        ];
        // 1 agent = 1 point. Still low (threshold is 3).
        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 1);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Low);
    }

    #[test]
    fn social_actions_push_to_medium() {
        let mut perception = solo_survival_perception();
        // Add 1 agent (1 point) + social actions (2 points) = 3 => Medium.
        perception.surroundings.agents_here = vec![
            VisibleAgent {
                name: "Friend".to_owned(),
                relationship: "friendly (0.6)".to_owned(),
                activity: "gathering".to_owned(),
            },
        ];
        perception.available_actions.push("communicate".to_owned());
        perception.available_actions.push("teach".to_owned());

        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 3);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Medium);
    }

    #[test]
    fn trade_and_messages_medium() {
        let mut perception = solo_survival_perception();
        // 1 agent (1) + 1 message (1) + trade actions (2) = 4 => Medium.
        perception.surroundings.agents_here = vec![
            VisibleAgent {
                name: "Trader".to_owned(),
                relationship: "neutral (0.0)".to_owned(),
                activity: "idle".to_owned(),
            },
        ];
        perception.surroundings.messages_here = vec![
            emergence_types::VisibleMessage {
                from: "Trader".to_owned(),
                tick: 9,
                content: "Want to trade?".to_owned(),
            },
        ];
        perception.available_actions.push("trade_offer".to_owned());

        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 4);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Medium);
    }

    #[test]
    fn governance_pushes_to_high() {
        let mut perception = solo_survival_perception();
        // 2 agents (2) + governance (3) + social (2) = 7 => High.
        perception.surroundings.agents_here = vec![
            VisibleAgent {
                name: "Leader".to_owned(),
                relationship: "friendly (0.8)".to_owned(),
                activity: "legislating".to_owned(),
            },
            VisibleAgent {
                name: "Follower".to_owned(),
                relationship: "neutral (0.3)".to_owned(),
                activity: "idle".to_owned(),
            },
        ];
        perception.available_actions.push("legislate".to_owned());
        perception.available_actions.push("communicate".to_owned());

        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 7);
        assert_eq!(score_complexity(&perception), ComplexityLevel::High);
    }

    #[test]
    fn storm_and_notifications_compound() {
        let mut perception = solo_survival_perception();
        // Storm (2) + 2 notifications (2) + structures (1) = 5 => Medium.
        perception.weather = Weather::Storm;
        perception.notifications = vec![
            "Winter is approaching".to_owned(),
            "Your shelter is damaged".to_owned(),
        ];
        perception.surroundings.structures_here = vec![
            emergence_types::VisibleStructure {
                structure_type: "shelter (basic hut)".to_owned(),
                owner: "Loner".to_owned(),
                durability: "30%".to_owned(),
                occupants: Vec::new(),
            },
        ];

        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 5);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Medium);
    }

    #[test]
    fn multiple_high_factors_stack() {
        let mut perception = solo_survival_perception();
        // 3 agents (3) + storm (2) + 3 notifications (3) + trade (2)
        // + social (2) + structures (1) + low health (1) + high hunger (1)
        // + many memories (1) = 16 => High.
        perception.surroundings.agents_here = vec![
            VisibleAgent {
                name: "A".to_owned(),
                relationship: "hostile (-0.5)".to_owned(),
                activity: "fighting".to_owned(),
            },
            VisibleAgent {
                name: "B".to_owned(),
                relationship: "friendly (0.7)".to_owned(),
                activity: "gathering".to_owned(),
            },
            VisibleAgent {
                name: "C".to_owned(),
                relationship: "neutral (0.0)".to_owned(),
                activity: "idle".to_owned(),
            },
        ];
        perception.weather = Weather::Storm;
        perception.notifications = vec![
            "Agent D has died".to_owned(),
            "Approaching winter".to_owned(),
            "Resource scarcity warning".to_owned(),
        ];
        perception.available_actions.push("trade_offer".to_owned());
        perception.available_actions.push("communicate".to_owned());
        perception.surroundings.structures_here = vec![
            emergence_types::VisibleStructure {
                structure_type: "campfire".to_owned(),
                owner: "Loner".to_owned(),
                durability: "80%".to_owned(),
                occupants: Vec::new(),
            },
        ];
        perception.self_state.health = 20;
        perception.self_state.hunger = 80;
        perception.recent_memory = vec![
            "Built a campfire".to_owned(),
            "Gathered wood".to_owned(),
            "Met agent B".to_owned(),
        ];

        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 16);
        assert_eq!(score_complexity(&perception), ComplexityLevel::High);
    }

    #[test]
    fn agent_cap_at_three() {
        let mut perception = solo_survival_perception();
        // 5 agents should still only contribute 3 points (capped).
        perception.surroundings.agents_here = vec![
            VisibleAgent { name: "A".to_owned(), relationship: "n".to_owned(), activity: "i".to_owned() },
            VisibleAgent { name: "B".to_owned(), relationship: "n".to_owned(), activity: "i".to_owned() },
            VisibleAgent { name: "C".to_owned(), relationship: "n".to_owned(), activity: "i".to_owned() },
            VisibleAgent { name: "D".to_owned(), relationship: "n".to_owned(), activity: "i".to_owned() },
            VisibleAgent { name: "E".to_owned(), relationship: "n".to_owned(), activity: "i".to_owned() },
        ];

        let raw = compute_raw_score(&perception);
        // Only 3 points from agents (capped), nothing else.
        assert_eq!(raw, 3);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Medium);
    }

    #[test]
    fn snow_counts_as_severe_weather() {
        let mut perception = solo_survival_perception();
        perception.weather = Weather::Snow;
        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 2);
        assert_eq!(score_complexity(&perception), ComplexityLevel::Low);
    }

    #[test]
    fn rain_and_drought_do_not_add_points() {
        let mut perception = solo_survival_perception();
        perception.weather = Weather::Rain;
        assert_eq!(compute_raw_score(&perception), 0);

        perception.weather = Weather::Drought;
        assert_eq!(compute_raw_score(&perception), 0);
    }

    #[test]
    fn display_trait_works() {
        assert_eq!(format!("{}", ComplexityLevel::Low), "low");
        assert_eq!(format!("{}", ComplexityLevel::Medium), "medium");
        assert_eq!(format!("{}", ComplexityLevel::High), "high");
    }

    #[test]
    fn message_cap_at_two() {
        let mut perception = solo_survival_perception();
        perception.surroundings.messages_here = vec![
            emergence_types::VisibleMessage { from: "A".to_owned(), tick: 1, content: "hi".to_owned() },
            emergence_types::VisibleMessage { from: "B".to_owned(), tick: 2, content: "hey".to_owned() },
            emergence_types::VisibleMessage { from: "C".to_owned(), tick: 3, content: "yo".to_owned() },
        ];
        // 3 messages, capped at 2 points.
        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 2);
    }

    #[test]
    fn notification_cap_at_three() {
        let mut perception = solo_survival_perception();
        perception.notifications = vec![
            "a".to_owned(),
            "b".to_owned(),
            "c".to_owned(),
            "d".to_owned(),
            "e".to_owned(),
        ];
        // 5 notifications, capped at 3 points.
        let raw = compute_raw_score(&perception);
        assert_eq!(raw, 3);
    }
}
