//! Conflict resolution for contested resources.
//!
//! When multiple agents try to gather the same scarce resource in a single
//! tick, the conflict resolution system decides who gets what. Per
//! `world-engine.md` section 7.3, the resolution strategies are:
//!
//! 1. **First-come-first-served**: agents are ordered by submission time,
//!    and each agent gathers in order until the resource is exhausted.
//! 2. **Splitting**: the available resource is divided equally among
//!    contenders, with remainder going to the first submitter.
//! 3. **Rejection**: agents who cannot receive any resource are rejected
//!    with [`RejectionReason::ConflictLost`].

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use emergence_types::{AgentId, RejectionReason, Resource};

/// The strategy used to resolve a conflict over a contested resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Agents are served in submission order until exhausted.
    FirstComeFirstServed,
    /// Resources are split equally among all contenders.
    EqualSplit,
}

/// A single gather claim from an agent for a resource at a location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatherClaim {
    /// The agent making the claim.
    pub agent_id: AgentId,
    /// The resource being claimed.
    pub resource: Resource,
    /// The quantity the agent wants to gather (from handler yield).
    pub requested: u32,
    /// When the action was submitted (for ordering).
    pub submitted_at: DateTime<Utc>,
}

/// The outcome of conflict resolution for a single agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// The agent receives the specified quantity.
    Granted {
        /// The quantity awarded.
        quantity: u32,
    },
    /// The agent receives nothing because the resource was exhausted.
    Rejected {
        /// Why the claim was rejected.
        reason: RejectionReason,
    },
}

/// Resolve conflicts among multiple agents claiming the same resource
/// at the same location.
///
/// `available` is the total quantity of the resource at the location.
/// `claims` are the gather claims, which will be sorted by submission time.
/// `strategy` determines how ties are broken.
///
/// Returns a map of agent ID to their claim outcome.
pub fn resolve_gather_conflict(
    available: u32,
    claims: &[GatherClaim],
    strategy: ConflictStrategy,
) -> BTreeMap<AgentId, ClaimOutcome> {
    if claims.is_empty() {
        return BTreeMap::new();
    }

    match strategy {
        ConflictStrategy::FirstComeFirstServed => {
            resolve_first_come_first_served(available, claims)
        }
        ConflictStrategy::EqualSplit => resolve_equal_split(available, claims),
    }
}

/// First-come-first-served: sort by submission time, grant in order.
fn resolve_first_come_first_served(
    available: u32,
    claims: &[GatherClaim],
) -> BTreeMap<AgentId, ClaimOutcome> {
    let mut sorted: Vec<&GatherClaim> = claims.iter().collect();
    sorted.sort_by_key(|c| c.submitted_at);

    let mut remaining = available;
    let mut outcomes = BTreeMap::new();

    for claim in sorted {
        if remaining == 0 {
            outcomes.insert(
                claim.agent_id,
                ClaimOutcome::Rejected {
                    reason: RejectionReason::ConflictLost,
                },
            );
            continue;
        }

        let granted = claim.requested.min(remaining);
        remaining = remaining.saturating_sub(granted);

        if granted == 0 {
            outcomes.insert(
                claim.agent_id,
                ClaimOutcome::Rejected {
                    reason: RejectionReason::ConflictLost,
                },
            );
        } else {
            outcomes.insert(claim.agent_id, ClaimOutcome::Granted { quantity: granted });
        }
    }

    outcomes
}

/// Equal split: divide available evenly, remainder goes to first submitter.
fn resolve_equal_split(
    available: u32,
    claims: &[GatherClaim],
) -> BTreeMap<AgentId, ClaimOutcome> {
    let mut sorted: Vec<&GatherClaim> = claims.iter().collect();
    sorted.sort_by_key(|c| c.submitted_at);

    let claim_count = u32::try_from(sorted.len()).unwrap_or(u32::MAX);
    if claim_count == 0 {
        return BTreeMap::new();
    }

    let base_share = available.checked_div(claim_count).unwrap_or(0);
    let leftover = available.checked_rem(claim_count).unwrap_or(0);

    let mut outcomes = BTreeMap::new();

    for (i, claim) in sorted.iter().enumerate() {
        // First agent gets the remainder
        let bonus = if i == 0 { leftover } else { 0 };
        let share = base_share.saturating_add(bonus);

        // Cap to what the agent actually requested
        let granted = share.min(claim.requested);

        if granted == 0 {
            outcomes.insert(
                claim.agent_id,
                ClaimOutcome::Rejected {
                    reason: RejectionReason::ConflictLost,
                },
            );
        } else {
            outcomes.insert(claim.agent_id, ClaimOutcome::Granted { quantity: granted });
        }
    }

    outcomes
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::Utc;
    use emergence_types::{AgentId, Resource};

    use super::*;

    fn make_claim(agent_id: AgentId, requested: u32, ms_offset: i64) -> GatherClaim {
        let base = Utc::now();
        let submitted_at = base
            .checked_add_signed(chrono::TimeDelta::milliseconds(ms_offset))
            .unwrap_or(base);
        GatherClaim {
            agent_id,
            resource: Resource::Wood,
            requested,
            submitted_at,
        }
    }

    #[test]
    fn single_agent_gets_all() {
        let agent = AgentId::new();
        let claims = vec![make_claim(agent, 5, 0)];

        let results =
            resolve_gather_conflict(10, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&agent),
            Some(&ClaimOutcome::Granted { quantity: 5 })
        );
    }

    #[test]
    fn single_agent_capped_by_available() {
        let agent = AgentId::new();
        let claims = vec![make_claim(agent, 10, 0)];

        let results = resolve_gather_conflict(3, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&agent),
            Some(&ClaimOutcome::Granted { quantity: 3 })
        );
    }

    #[test]
    fn first_come_first_served_ordering() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 5, 0),   // First submitter
            make_claim(a2, 5, 100), // Second submitter
        ];

        // Only 6 available: a1 gets 5, a2 gets 1
        let results = resolve_gather_conflict(6, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 5 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Granted { quantity: 1 })
        );
    }

    #[test]
    fn first_come_rejects_late_arrivals() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 5, 0),
            make_claim(a2, 5, 100),
        ];

        // Only 5 available: a1 gets all, a2 gets nothing
        let results = resolve_gather_conflict(5, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 5 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Rejected {
                reason: RejectionReason::ConflictLost,
            })
        );
    }

    #[test]
    fn equal_split_divides_evenly() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 5, 0),
            make_claim(a2, 5, 100),
        ];

        // 10 available, 2 agents: each gets 5
        let results = resolve_gather_conflict(10, &claims, ConflictStrategy::EqualSplit);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 5 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Granted { quantity: 5 })
        );
    }

    #[test]
    fn equal_split_remainder_to_first() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 10, 0),
            make_claim(a2, 10, 100),
        ];

        // 7 available, 2 agents: base=3, remainder=1
        // a1 gets 3+1=4, a2 gets 3
        let results = resolve_gather_conflict(7, &claims, ConflictStrategy::EqualSplit);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 4 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Granted { quantity: 3 })
        );
    }

    #[test]
    fn equal_split_capped_by_request() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 2, 0),   // Only wants 2
            make_claim(a2, 10, 100),
        ];

        // 20 available, base=10 each
        // a1 wants 2, gets min(10+0, 2) = 2
        // a2 wants 10, gets min(10, 10) = 10
        let results = resolve_gather_conflict(20, &claims, ConflictStrategy::EqualSplit);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 2 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Granted { quantity: 10 })
        );
    }

    #[test]
    fn zero_available_rejects_all() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let claims = vec![
            make_claim(a1, 5, 0),
            make_claim(a2, 5, 100),
        ];

        let results = resolve_gather_conflict(0, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Rejected {
                reason: RejectionReason::ConflictLost,
            })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Rejected {
                reason: RejectionReason::ConflictLost,
            })
        );
    }

    #[test]
    fn empty_claims_returns_empty() {
        let results: BTreeMap<AgentId, ClaimOutcome> =
            resolve_gather_conflict(100, &[], ConflictStrategy::FirstComeFirstServed);
        assert!(results.is_empty());
    }

    #[test]
    fn three_agents_first_come() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let a3 = AgentId::new();
        let claims = vec![
            make_claim(a1, 3, 0),
            make_claim(a2, 3, 100),
            make_claim(a3, 3, 200),
        ];

        // 5 available: a1 gets 3, a2 gets 2, a3 gets 0
        let results = resolve_gather_conflict(5, &claims, ConflictStrategy::FirstComeFirstServed);
        assert_eq!(
            results.get(&a1),
            Some(&ClaimOutcome::Granted { quantity: 3 })
        );
        assert_eq!(
            results.get(&a2),
            Some(&ClaimOutcome::Granted { quantity: 2 })
        );
        assert_eq!(
            results.get(&a3),
            Some(&ClaimOutcome::Rejected {
                reason: RejectionReason::ConflictLost,
            })
        );
    }
}
