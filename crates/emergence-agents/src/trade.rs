//! Trading system for the Emergence simulation.
//!
//! Implements the full trade lifecycle from `world-engine.md` section 7.1
//! and `data-schemas.md` sections 5.4 and 7.2:
//!
//! 1. [`trade_offer`] -- Agent proposes a resource exchange (energy cost: 2).
//! 2. [`trade_accept`] -- Target agent accepts and executes the swap (energy cost: 0).
//! 3. [`trade_reject`] -- Target agent declines the offer (energy cost: 0).
//! 4. [`expire_trades`] -- Remove trades past their `expires_at_tick`.
//!
//! # Ledger Integration
//!
//! A successful trade produces one [`LedgerEntry`] per resource per direction
//! via [`Ledger::record_agent_transfer`]. The conservation law is maintained
//! because every resource debited from one agent is credited to the other.
//!
//! [`LedgerEntry`]: emergence_types::LedgerEntry
//! [`Ledger::record_agent_transfer`]: emergence_ledger::Ledger::record_agent_transfer

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use emergence_ledger::{AgentTransferParams, Ledger, LedgerError};
use emergence_types::{
    ActionOutcome, ActionType, AgentId, AgentState, PendingTrade, Resource,
    TradeCompletedDetails, TradeFailReason, TradeFailedDetails, TradeId,
};

use crate::actions::costs;
use crate::error::AgentError;
use crate::inventory;
use crate::vitals;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default number of ticks a trade offer remains pending before expiring.
pub const DEFAULT_TRADE_EXPIRY_TICKS: u64 = 3;

// ---------------------------------------------------------------------------
// Trade offer
// ---------------------------------------------------------------------------

/// Create a pending trade from an offerer to a target agent.
///
/// Validates that:
/// - The offer and request maps are non-empty.
/// - The offerer has the offered resources in inventory.
/// - The offerer has enough energy (2).
///
/// On success, deducts energy from the offerer and returns a [`PendingTrade`]
/// ready to be stored in `Dragonfly` along with the action outcome.
///
/// The caller is responsible for verifying co-location (validation pipeline
/// stage 3) before calling this function.
pub fn trade_offer(
    offerer: &mut AgentState,
    target_id: AgentId,
    offer: &BTreeMap<Resource, u32>,
    request: &BTreeMap<Resource, u32>,
    current_tick: u64,
    expiry_ticks: u64,
) -> Result<(PendingTrade, ActionOutcome), AgentError> {
    // Validate non-empty maps
    if offer.is_empty() {
        return Err(AgentError::ArithmeticOverflow {
            context: String::from("trade offer map is empty"),
        });
    }
    if request.is_empty() {
        return Err(AgentError::ArithmeticOverflow {
            context: String::from("trade request map is empty"),
        });
    }

    // Validate offerer has the offered resources
    for (resource, &quantity) in offer {
        if !inventory::has_resource(&offerer.inventory, *resource, quantity) {
            let available = offerer.inventory.get(resource).copied().unwrap_or(0);
            return Err(AgentError::InsufficientResource {
                resource: *resource,
                requested: quantity,
                available,
            });
        }
    }

    // Deduct energy
    vitals::apply_energy_cost(offerer, costs::energy_cost(ActionType::TradeOffer));

    // Build pending trade
    let trade_id = TradeId::new();
    let expires_at_tick = current_tick
        .checked_add(expiry_ticks)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("trade expiry tick overflow"),
        })?;

    let pending = PendingTrade {
        trade_id,
        offerer_id: offerer.agent_id,
        target_id,
        offered_resources: offer.clone(),
        requested_resources: request.clone(),
        created_at_tick: current_tick,
        expires_at_tick,
        location_id: offerer.location_id,
    };

    let outcome = ActionOutcome {
        resource_changes: BTreeMap::new(),
        energy_spent: costs::energy_cost(ActionType::TradeOffer),
        skill_xp: BTreeMap::new(),
        details: serde_json::json!({
            "trade_id": trade_id.to_string(),
            "target": target_id.to_string(),
            "offered": format!("{offer:?}"),
            "requested": format!("{request:?}"),
            "expires_at_tick": expires_at_tick,
        }),
    };

    Ok((pending, outcome))
}

// ---------------------------------------------------------------------------
// Trade accept
// ---------------------------------------------------------------------------

/// Result of executing a trade acceptance.
pub struct TradeAcceptResult {
    /// The action outcome to return to the accepting agent.
    pub outcome: ActionOutcome,
    /// Completed trade details for event emission.
    pub completed: TradeCompletedDetails,
}

/// Accept a pending trade and execute the resource swap through the ledger.
///
/// Validates that:
/// - The target agent has the requested resources.
/// - Both agents are still at the same location as the trade.
///
/// On success, transfers resources between inventories and records ledger
/// entries for every resource in both directions.
///
/// # Errors
///
/// Returns [`AgentError`] if the target lacks resources or arithmetic fails.
/// Returns [`LedgerError`] (wrapped) if a ledger entry fails validation.
pub fn trade_accept(
    offerer: &mut AgentState,
    target: &mut AgentState,
    trade: &PendingTrade,
    ledger: &mut Ledger,
    current_tick: u64,
) -> Result<TradeAcceptResult, TradeError> {
    // Verify co-location
    if offerer.location_id != trade.location_id || target.location_id != trade.location_id {
        return Err(TradeError::NotCoLocated);
    }

    // Verify both sides have the required resources (before any mutations)
    validate_trade_inventories(offerer, target, trade)?;

    // Deduct energy (0 for accept, but apply for consistency)
    vitals::apply_energy_cost(target, costs::energy_cost(ActionType::TradeAccept));

    // Execute bidirectional resource transfers and record ledger entries
    execute_resource_transfers(offerer, target, trade, ledger, current_tick)?;

    // Build the outcome for the accepting agent and the completed details
    build_accept_outcome(trade)
}

/// Validate that both agents still hold the resources required for the trade.
///
/// Checks the offerer's inventory against `offered_resources` and the
/// target's inventory against `requested_resources`. No mutations occur.
fn validate_trade_inventories(
    offerer: &AgentState,
    target: &AgentState,
    trade: &PendingTrade,
) -> Result<(), TradeError> {
    for (resource, &quantity) in &trade.offered_resources {
        if !inventory::has_resource(&offerer.inventory, *resource, quantity) {
            return Err(TradeError::OffererInsufficientResources {
                resource: *resource,
                needed: quantity,
                available: offerer.inventory.get(resource).copied().unwrap_or(0),
            });
        }
    }

    for (resource, &quantity) in &trade.requested_resources {
        if !inventory::has_resource(&target.inventory, *resource, quantity) {
            return Err(TradeError::TargetInsufficientResources {
                resource: *resource,
                needed: quantity,
                available: target.inventory.get(resource).copied().unwrap_or(0),
            });
        }
    }

    Ok(())
}

/// Execute bidirectional inventory transfers and record ledger entries.
///
/// Transfers offered resources from offerer to target, and requested
/// resources from target to offerer, recording a [`LedgerEntry`] for
/// each resource direction.
///
/// [`LedgerEntry`]: emergence_types::LedgerEntry
fn execute_resource_transfers(
    offerer: &mut AgentState,
    target: &mut AgentState,
    trade: &PendingTrade,
    ledger: &mut Ledger,
    current_tick: u64,
) -> Result<(), TradeError> {
    let trade_ref_id = trade.trade_id.into_inner();

    // Offerer -> target for offered resources
    for (resource, &quantity) in &trade.offered_resources {
        transfer_single_resource(
            &mut offerer.inventory,
            &mut target.inventory,
            target.carry_capacity,
            *resource,
            quantity,
        )?;
        record_trade_ledger_entry(
            ledger,
            current_tick,
            *resource,
            quantity,
            offerer.agent_id,
            target.agent_id,
            trade_ref_id,
        )?;
    }

    // Target -> offerer for requested resources
    for (resource, &quantity) in &trade.requested_resources {
        transfer_single_resource(
            &mut target.inventory,
            &mut offerer.inventory,
            offerer.carry_capacity,
            *resource,
            quantity,
        )?;
        record_trade_ledger_entry(
            ledger,
            current_tick,
            *resource,
            quantity,
            target.agent_id,
            offerer.agent_id,
            trade_ref_id,
        )?;
    }

    Ok(())
}

/// Move a single resource between two inventories.
fn transfer_single_resource(
    from_inv: &mut BTreeMap<Resource, u32>,
    to_inv: &mut BTreeMap<Resource, u32>,
    to_capacity: u32,
    resource: Resource,
    quantity: u32,
) -> Result<(), TradeError> {
    inventory::remove_resource(from_inv, resource, quantity).map_err(TradeError::Agent)?;
    inventory::add_resource(to_inv, to_capacity, resource, quantity).map_err(TradeError::Agent)?;
    Ok(())
}

/// Record a single agent-to-agent transfer in the ledger.
fn record_trade_ledger_entry(
    ledger: &mut Ledger,
    tick: u64,
    resource: Resource,
    quantity: u32,
    from: AgentId,
    to: AgentId,
    reference_id: uuid::Uuid,
) -> Result<(), TradeError> {
    let decimal_qty = u64::from(quantity);
    ledger
        .record_agent_transfer(AgentTransferParams {
            tick,
            resource,
            quantity: Decimal::from(decimal_qty),
            from_agent: from.into_inner(),
            to_agent: to.into_inner(),
            reason: "TRADE".to_owned(),
            reference_id: Some(reference_id),
        })
        .map(|_entry| ())
        .map_err(TradeError::Ledger)
}

/// Build the [`ActionOutcome`] and [`TradeCompletedDetails`] for a successful
/// trade acceptance (from the target agent's perspective).
fn build_accept_outcome(trade: &PendingTrade) -> Result<TradeAcceptResult, TradeError> {
    let mut resource_changes = BTreeMap::new();

    // From target's perspective: they gave requested, received offered
    for (resource, &quantity) in &trade.requested_resources {
        let neg = i64::from(quantity).checked_neg().ok_or_else(|| {
            TradeError::Agent(AgentError::ArithmeticOverflow {
                context: String::from("trade resource change negation overflow"),
            })
        })?;
        resource_changes.insert(*resource, neg);
    }
    for (resource, &quantity) in &trade.offered_resources {
        let pos = i64::from(quantity);
        let entry = resource_changes.entry(*resource).or_insert(0);
        *entry = entry.checked_add(pos).ok_or_else(|| {
            TradeError::Agent(AgentError::ArithmeticOverflow {
                context: String::from("trade resource change addition overflow"),
            })
        })?;
    }

    let outcome = ActionOutcome {
        resource_changes,
        energy_spent: costs::energy_cost(ActionType::TradeAccept),
        skill_xp: BTreeMap::new(),
        details: serde_json::json!({
            "trade_id": trade.trade_id.to_string(),
            "offerer": trade.offerer_id.to_string(),
            "received": format!("{:?}", trade.offered_resources),
            "gave": format!("{:?}", trade.requested_resources),
        }),
    };

    let completed = TradeCompletedDetails {
        trade_id: trade.trade_id,
        agent_a: trade.offerer_id,
        agent_b: trade.target_id,
        gave: trade.offered_resources.clone(),
        received: trade.requested_resources.clone(),
    };

    Ok(TradeAcceptResult {
        outcome,
        completed,
    })
}

// ---------------------------------------------------------------------------
// Trade reject
// ---------------------------------------------------------------------------

/// Reject a pending trade.
///
/// Returns a [`TradeFailedDetails`] for event emission. Energy cost is 0.
/// The caller is responsible for deleting the trade from `Dragonfly`.
pub fn trade_reject(
    target: &mut AgentState,
    trade: &PendingTrade,
) -> (ActionOutcome, TradeFailedDetails) {
    // Deduct energy (0 for reject)
    vitals::apply_energy_cost(target, costs::energy_cost(ActionType::TradeReject));

    let outcome = ActionOutcome {
        resource_changes: BTreeMap::new(),
        energy_spent: costs::energy_cost(ActionType::TradeReject),
        skill_xp: BTreeMap::new(),
        details: serde_json::json!({
            "trade_id": trade.trade_id.to_string(),
            "offerer": trade.offerer_id.to_string(),
            "action": "rejected",
        }),
    };

    let failed = TradeFailedDetails {
        trade_id: trade.trade_id,
        reason: TradeFailReason::Rejected,
        offerer_id: trade.offerer_id,
        target_id: trade.target_id,
    };

    (outcome, failed)
}

// ---------------------------------------------------------------------------
// Trade expiry
// ---------------------------------------------------------------------------

/// Check whether a pending trade has expired based on the current tick.
pub const fn is_trade_expired(trade: &PendingTrade, current_tick: u64) -> bool {
    current_tick >= trade.expires_at_tick
}

/// Build a [`TradeFailedDetails`] for an expired trade.
pub const fn expire_trade(trade: &PendingTrade) -> TradeFailedDetails {
    TradeFailedDetails {
        trade_id: trade.trade_id,
        reason: TradeFailReason::Expired,
        offerer_id: trade.offerer_id,
        target_id: trade.target_id,
    }
}

// ---------------------------------------------------------------------------
// Trade validation helpers (for the validation pipeline)
// ---------------------------------------------------------------------------

/// Validate a `TradeOffer` action: target must be co-located with offerer.
///
/// Returns `Ok(())` if the target agent is in the `agents_at_location` list.
pub fn validate_trade_offer_location(
    target_agent: AgentId,
    agents_at_location: &[AgentId],
) -> Result<(), emergence_types::RejectionReason> {
    if agents_at_location.contains(&target_agent) {
        Ok(())
    } else {
        Err(emergence_types::RejectionReason::InvalidTarget)
    }
}

/// Validate a `TradeOffer` action: offerer must have the offered resources.
pub fn validate_trade_offer_resources(
    offerer_inventory: &BTreeMap<Resource, u32>,
    offer: &BTreeMap<Resource, u32>,
) -> Result<(), emergence_types::RejectionReason> {
    for (resource, &quantity) in offer {
        if !inventory::has_resource(offerer_inventory, *resource, quantity) {
            return Err(emergence_types::RejectionReason::InsufficientResources);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors specific to trade operations.
#[derive(Debug, thiserror::Error)]
pub enum TradeError {
    /// The offerer does not have enough of a resource.
    #[error(
        "offerer lacks {resource:?}: needs {needed}, has {available}"
    )]
    OffererInsufficientResources {
        /// The resource in question.
        resource: Resource,
        /// Quantity needed.
        needed: u32,
        /// Quantity available.
        available: u32,
    },

    /// The target does not have enough of a resource.
    #[error(
        "target lacks {resource:?}: needs {needed}, has {available}"
    )]
    TargetInsufficientResources {
        /// The resource in question.
        resource: Resource,
        /// Quantity needed.
        needed: u32,
        /// Quantity available.
        available: u32,
    },

    /// The agents are not at the same location as the trade.
    #[error("agents are not co-located for trade execution")]
    NotCoLocated,

    /// An agent inventory or vitals error occurred during the swap.
    #[error("agent error during trade: {0}")]
    Agent(#[from] AgentError),

    /// A ledger recording error occurred during the swap.
    #[error("ledger error during trade: {0}")]
    Ledger(LedgerError),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_ledger::conservation::ConservationResult;
    use emergence_types::{AgentId, LocationId, Resource};

    use super::*;

    fn make_agent(energy: u32, location: LocationId) -> AgentState {
        AgentState {
            agent_id: AgentId::new(),
            energy,
            health: 100,
            hunger: 0,
            age: 0,
            born_at_tick: 0,
            location_id: location,
            destination_id: None,
            travel_progress: 0,
            inventory: BTreeMap::new(),
            carry_capacity: 100,
            knowledge: BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // trade_offer tests
    // -----------------------------------------------------------------------

    #[test]
    fn trade_offer_creates_pending_trade() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 10);
        let target_id = AgentId::new();

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = trade_offer(
            &mut offerer,
            target_id,
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        );

        assert!(result.is_ok());
        let (pending, outcome) = result.unwrap();

        assert_eq!(pending.offerer_id, offerer.agent_id);
        assert_eq!(pending.target_id, target_id);
        assert_eq!(pending.offered_resources, offer);
        assert_eq!(pending.requested_resources, request);
        assert_eq!(pending.created_at_tick, 1);
        assert_eq!(pending.expires_at_tick, 4); // 1 + 3
        assert_eq!(pending.location_id, loc);
        assert_eq!(outcome.energy_spent, 2);
        assert_eq!(offerer.energy, 78); // 80 - 2
    }

    #[test]
    fn trade_offer_rejects_insufficient_resources() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 2); // only 2

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5); // wants to offer 5
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = trade_offer(
            &mut offerer,
            AgentId::new(),
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        );

        assert!(result.is_err());
    }

    #[test]
    fn trade_offer_rejects_empty_offer() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        let offer = BTreeMap::new(); // empty
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = trade_offer(
            &mut offerer,
            AgentId::new(),
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        );

        assert!(result.is_err());
    }

    #[test]
    fn trade_offer_rejects_empty_request() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 5);
        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let request = BTreeMap::new(); // empty

        let result = trade_offer(
            &mut offerer,
            AgentId::new(),
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        );

        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // trade_accept tests
    // -----------------------------------------------------------------------

    #[test]
    fn trade_accept_swaps_resources() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 10);

        let mut target = make_agent(80, loc);
        target.inventory.insert(Resource::Stone, 10);

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let mut ledger = Ledger::new();
        let result = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);

        assert!(result.is_ok());

        // Offerer: had 10 wood, gave 5, got 3 stone
        assert_eq!(offerer.inventory.get(&Resource::Wood).copied(), Some(5));
        assert_eq!(offerer.inventory.get(&Resource::Stone).copied(), Some(3));

        // Target: had 10 stone, gave 3, got 5 wood
        assert_eq!(target.inventory.get(&Resource::Stone).copied(), Some(7));
        assert_eq!(target.inventory.get(&Resource::Wood).copied(), Some(5));

        // Ledger should have 2 entries (wood offerer->target, stone target->offerer)
        assert_eq!(ledger.len(), 2);
    }

    #[test]
    fn trade_accept_ledger_balances() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 10);
        offerer.inventory.insert(Resource::FoodBerry, 5);

        let mut target = make_agent(80, loc);
        target.inventory.insert(Resource::Stone, 10);

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        offered.insert(Resource::FoodBerry, 2);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let mut ledger = Ledger::new();
        let result = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);
        assert!(result.is_ok());

        // 3 entries: wood, food_berry from offerer->target; stone from target->offerer
        assert_eq!(ledger.len(), 3);

        // Conservation law must hold
        assert_eq!(
            ledger.verify_conservation(2),
            ConservationResult::Balanced
        );
    }

    #[test]
    fn trade_accept_rejects_not_co_located() {
        let loc_a = LocationId::new();
        let loc_b = LocationId::new();

        let mut offerer = make_agent(80, loc_a);
        offerer.inventory.insert(Resource::Wood, 10);

        let mut target = make_agent(80, loc_b); // Different location
        target.inventory.insert(Resource::Stone, 10);

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc_a,
        };

        let mut ledger = Ledger::new();
        let result = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);

        assert!(result.is_err());
        assert!(matches!(result.err().unwrap(), TradeError::NotCoLocated));
    }

    #[test]
    fn trade_accept_rejects_target_insufficient_resources() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 10);

        let mut target = make_agent(80, loc);
        target.inventory.insert(Resource::Stone, 2); // Only 2, needs 3

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let mut ledger = Ledger::new();
        let result = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            TradeError::TargetInsufficientResources { .. }
        ));
    }

    #[test]
    fn trade_accept_rejects_offerer_insufficient_resources() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 2); // Only 2, promised 5

        let mut target = make_agent(80, loc);
        target.inventory.insert(Resource::Stone, 10);

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let mut ledger = Ledger::new();
        let result = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            TradeError::OffererInsufficientResources { .. }
        ));
    }

    #[test]
    fn trade_accept_no_inventory_changes_on_validation_failure() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 10);

        let mut target = make_agent(80, loc);
        target.inventory.insert(Resource::Stone, 1); // Insufficient

        let mut offered = BTreeMap::new();
        offered.insert(Resource::Wood, 5);
        let mut requested = BTreeMap::new();
        requested.insert(Resource::Stone, 3);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: offerer.agent_id,
            target_id: target.agent_id,
            offered_resources: offered,
            requested_resources: requested,
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let mut ledger = Ledger::new();
        let _ = trade_accept(&mut offerer, &mut target, &trade, &mut ledger, 2);

        // No changes should have been made
        assert_eq!(
            offerer.inventory.get(&Resource::Wood).copied(),
            Some(10)
        );
        assert_eq!(
            target.inventory.get(&Resource::Stone).copied(),
            Some(1)
        );
        assert!(ledger.is_empty());
    }

    // -----------------------------------------------------------------------
    // trade_reject tests
    // -----------------------------------------------------------------------

    #[test]
    fn trade_reject_produces_failed_details() {
        let loc = LocationId::new();
        let mut target = make_agent(80, loc);

        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: AgentId::new(),
            target_id: target.agent_id,
            offered_resources: BTreeMap::new(),
            requested_resources: BTreeMap::new(),
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: loc,
        };

        let (outcome, failed) = trade_reject(&mut target, &trade);

        assert_eq!(outcome.energy_spent, 0);
        assert_eq!(failed.reason, TradeFailReason::Rejected);
        assert_eq!(failed.trade_id, trade.trade_id);
        assert_eq!(failed.offerer_id, trade.offerer_id);
        assert_eq!(failed.target_id, target.agent_id);
    }

    // -----------------------------------------------------------------------
    // Trade expiry tests
    // -----------------------------------------------------------------------

    #[test]
    fn is_trade_expired_not_yet() {
        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: AgentId::new(),
            target_id: AgentId::new(),
            offered_resources: BTreeMap::new(),
            requested_resources: BTreeMap::new(),
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: LocationId::new(),
        };

        assert!(!is_trade_expired(&trade, 1));
        assert!(!is_trade_expired(&trade, 2));
        assert!(!is_trade_expired(&trade, 3));
    }

    #[test]
    fn is_trade_expired_at_expiry() {
        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: AgentId::new(),
            target_id: AgentId::new(),
            offered_resources: BTreeMap::new(),
            requested_resources: BTreeMap::new(),
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: LocationId::new(),
        };

        assert!(is_trade_expired(&trade, 4));
        assert!(is_trade_expired(&trade, 5));
    }

    #[test]
    fn expire_trade_produces_failed_details() {
        let trade = PendingTrade {
            trade_id: TradeId::new(),
            offerer_id: AgentId::new(),
            target_id: AgentId::new(),
            offered_resources: BTreeMap::new(),
            requested_resources: BTreeMap::new(),
            created_at_tick: 1,
            expires_at_tick: 4,
            location_id: LocationId::new(),
        };

        let failed = expire_trade(&trade);
        assert_eq!(failed.reason, TradeFailReason::Expired);
        assert_eq!(failed.trade_id, trade.trade_id);
    }

    // -----------------------------------------------------------------------
    // Validation helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_trade_offer_location_co_located() {
        let target = AgentId::new();
        let agents = vec![AgentId::new(), target, AgentId::new()];
        assert!(validate_trade_offer_location(target, &agents).is_ok());
    }

    #[test]
    fn validate_trade_offer_location_not_present() {
        let target = AgentId::new();
        let agents = vec![AgentId::new(), AgentId::new()];
        assert_eq!(
            validate_trade_offer_location(target, &agents),
            Err(emergence_types::RejectionReason::InvalidTarget)
        );
    }

    #[test]
    fn validate_trade_offer_resources_sufficient() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Wood, 10);
        inv.insert(Resource::Stone, 5);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        offer.insert(Resource::Stone, 3);

        assert!(validate_trade_offer_resources(&inv, &offer).is_ok());
    }

    #[test]
    fn validate_trade_offer_resources_insufficient() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Wood, 2);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);

        assert_eq!(
            validate_trade_offer_resources(&inv, &offer),
            Err(emergence_types::RejectionReason::InsufficientResources)
        );
    }

    // -----------------------------------------------------------------------
    // Full trade cycle test
    // -----------------------------------------------------------------------

    #[test]
    fn full_trade_cycle_offer_accept() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 20);

        let target_id = AgentId::new();
        let mut target = make_agent(80, loc);
        target.agent_id = target_id;
        target.inventory.insert(Resource::Stone, 15);

        // Step 1: Offer
        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 8);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 5);

        let (pending, _offer_outcome) = trade_offer(
            &mut offerer,
            target_id,
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        )
        .unwrap();

        // Offerer energy: 80 - 2 = 78
        assert_eq!(offerer.energy, 78);

        // Step 2: Accept
        let mut ledger = Ledger::new();
        let accept_result =
            trade_accept(&mut offerer, &mut target, &pending, &mut ledger, 2).unwrap();

        // Verify resource swap
        assert_eq!(offerer.inventory.get(&Resource::Wood).copied(), Some(12));
        assert_eq!(offerer.inventory.get(&Resource::Stone).copied(), Some(5));
        assert_eq!(target.inventory.get(&Resource::Stone).copied(), Some(10));
        assert_eq!(target.inventory.get(&Resource::Wood).copied(), Some(8));

        // Verify completed details
        assert_eq!(accept_result.completed.trade_id, pending.trade_id);
        assert_eq!(accept_result.completed.agent_a, offerer.agent_id);
        assert_eq!(accept_result.completed.agent_b, target_id);

        // Verify ledger (2 entries: wood offerer->target, stone target->offerer)
        assert_eq!(ledger.len(), 2);
        assert_eq!(
            ledger.verify_conservation(2),
            ConservationResult::Balanced
        );
    }

    #[test]
    fn full_trade_cycle_offer_reject() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 20);

        let target_id = AgentId::new();
        let mut target = make_agent(80, loc);
        target.agent_id = target_id;
        target.inventory.insert(Resource::Stone, 15);

        // Step 1: Offer
        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 8);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 5);

        let (pending, _) = trade_offer(
            &mut offerer,
            target_id,
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        )
        .unwrap();

        // Step 2: Reject
        let (_outcome, failed) = trade_reject(&mut target, &pending);

        assert_eq!(failed.reason, TradeFailReason::Rejected);

        // No inventory changes
        assert_eq!(offerer.inventory.get(&Resource::Wood).copied(), Some(20));
        assert_eq!(target.inventory.get(&Resource::Stone).copied(), Some(15));
    }

    #[test]
    fn full_trade_cycle_offer_expire() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 20);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 8);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 5);

        let (pending, _) = trade_offer(
            &mut offerer,
            AgentId::new(),
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        )
        .unwrap();

        // Not expired at tick 3
        assert!(!is_trade_expired(&pending, 3));

        // Expired at tick 4
        assert!(is_trade_expired(&pending, 4));

        let failed = expire_trade(&pending);
        assert_eq!(failed.reason, TradeFailReason::Expired);

        // No inventory changes
        assert_eq!(offerer.inventory.get(&Resource::Wood).copied(), Some(20));
    }

    #[test]
    fn trade_with_multiple_resources_both_directions() {
        let loc = LocationId::new();
        let mut offerer = make_agent(80, loc);
        offerer.inventory.insert(Resource::Wood, 20);
        offerer.inventory.insert(Resource::FoodBerry, 10);

        let target_id = AgentId::new();
        let mut target = make_agent(80, loc);
        target.agent_id = target_id;
        target.inventory.insert(Resource::Stone, 15);
        target.inventory.insert(Resource::Water, 8);

        // Offer: 5 wood + 3 berry for 4 stone + 2 water
        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        offer.insert(Resource::FoodBerry, 3);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 4);
        request.insert(Resource::Water, 2);

        let (pending, _) = trade_offer(
            &mut offerer,
            target_id,
            &offer,
            &request,
            1,
            DEFAULT_TRADE_EXPIRY_TICKS,
        )
        .unwrap();

        let mut ledger = Ledger::new();
        let result =
            trade_accept(&mut offerer, &mut target, &pending, &mut ledger, 2).unwrap();

        // Offerer: 20-5=15 wood, 10-3=7 berry, +4 stone, +2 water
        assert_eq!(offerer.inventory.get(&Resource::Wood).copied(), Some(15));
        assert_eq!(
            offerer.inventory.get(&Resource::FoodBerry).copied(),
            Some(7)
        );
        assert_eq!(offerer.inventory.get(&Resource::Stone).copied(), Some(4));
        assert_eq!(offerer.inventory.get(&Resource::Water).copied(), Some(2));

        // Target: +5 wood, +3 berry, 15-4=11 stone, 8-2=6 water
        assert_eq!(target.inventory.get(&Resource::Wood).copied(), Some(5));
        assert_eq!(
            target.inventory.get(&Resource::FoodBerry).copied(),
            Some(3)
        );
        assert_eq!(target.inventory.get(&Resource::Stone).copied(), Some(11));
        assert_eq!(target.inventory.get(&Resource::Water).copied(), Some(6));

        // 4 ledger entries: wood, berry (offerer->target), stone, water (target->offerer)
        assert_eq!(ledger.len(), 4);
        assert_eq!(
            ledger.verify_conservation(2),
            ConservationResult::Balanced
        );

        // Completed details
        assert_eq!(result.completed.agent_a, offerer.agent_id);
        assert_eq!(result.completed.agent_b, target_id);
    }
}
