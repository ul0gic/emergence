//! The central ledger: an append-only log of all resource transfers.
//!
//! The [`Ledger`] struct is the in-memory representation of the ledger
//! for the current simulation run. It holds all [`LedgerEntry`] values
//! and provides methods for recording transactions, querying balances,
//! and verifying the conservation law.
//!
//! # Design
//!
//! - **Append-only**: entries are never modified or deleted.
//! - **Double-entry**: every transfer has a debit (from) and credit (to).
//! - **Conservation**: total resources in == total resources out per tick.
//! - **Precision**: all quantities use [`Decimal`] -- no floating point.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use uuid::Uuid;

use emergence_types::{EntityType, LedgerEntry, LedgerEntryType, Resource};

use crate::conservation::{verify_conservation, verify_conservation_strict, ConservationResult};
use crate::{LedgerError, TransactionBuilder};

// ---------------------------------------------------------------------------
// Transfer parameters
// ---------------------------------------------------------------------------

/// Parameters for recording an agent-to-agent resource transfer.
pub struct AgentTransferParams {
    /// The tick number.
    pub tick: u64,
    /// The resource being transferred.
    pub resource: Resource,
    /// Quantity transferred.
    pub quantity: Decimal,
    /// Source agent UUID.
    pub from_agent: Uuid,
    /// Destination agent UUID.
    pub to_agent: Uuid,
    /// Human-readable reason (e.g. "TRADE", "GIFT").
    pub reason: String,
    /// Optional reference to a related entity (e.g. trade ID).
    pub reference_id: Option<Uuid>,
}

/// Parameters for recording a general ledger transfer.
///
/// Packs the many arguments of a transfer into a single struct to satisfy
/// clippy's argument count limit and improve call-site readability.
pub struct TransferParams {
    /// The tick number.
    pub tick: u64,
    /// The category of transfer.
    pub entry_type: LedgerEntryType,
    /// The resource being transferred.
    pub resource: Resource,
    /// Quantity transferred.
    pub quantity: Decimal,
    /// Source entity UUID.
    pub from_entity: Uuid,
    /// Source entity type.
    pub from_entity_type: EntityType,
    /// Destination entity UUID.
    pub to_entity: Uuid,
    /// Destination entity type.
    pub to_entity_type: EntityType,
    /// Human-readable reason.
    pub reason: String,
    /// Optional reference to a related entity.
    pub reference_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// Ledger
// ---------------------------------------------------------------------------

/// The central ledger tracking all resource transfers in the simulation.
///
/// Every resource movement -- regeneration, gathering, consumption, trading,
/// building, salvaging, decay, dropping, and picking up -- produces one or
/// more [`LedgerEntry`] records appended to this ledger.
///
/// The ledger enforces three invariants:
/// 1. All quantities are positive (validated at entry creation).
/// 2. Every entry type has the correct source/destination entity types.
/// 3. The conservation law holds at the end of every tick.
#[derive(Debug, Default)]
pub struct Ledger {
    /// All entries, in insertion order.
    entries: Vec<LedgerEntry>,
}

impl Ledger {
    /// Create a new empty ledger.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Return the number of entries in the ledger.
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the ledger has no entries.
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append a pre-built [`LedgerEntry`] to the ledger.
    ///
    /// This is for entries that were constructed externally (e.g. loaded
    /// from the database). For new entries, prefer [`record_transfer`],
    /// [`record_regeneration`], [`record_consumption`], etc.
    ///
    /// [`record_transfer`]: Ledger::record_transfer
    /// [`record_regeneration`]: Ledger::record_regeneration
    /// [`record_consumption`]: Ledger::record_consumption
    pub fn append(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }

    /// Record a resource transfer between two entities.
    ///
    /// This is the general-purpose recording method. It builds and validates
    /// a [`LedgerEntry`] via the [`TransactionBuilder`] and appends it.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_transfer(
        &mut self,
        params: TransferParams,
    ) -> Result<&LedgerEntry, LedgerError> {
        let mut builder = TransactionBuilder::new(params.tick, params.entry_type, params.resource)
            .from(params.from_entity, params.from_entity_type)
            .to(params.to_entity, params.to_entity_type)
            .quantity(params.quantity)
            .reason(params.reason);

        if let Some(ref_id) = params.reference_id {
            builder = builder.reference_id(ref_id);
        }

        let entry = builder.build()?;
        self.entries.push(entry);

        // Return a reference to the entry we just pushed.
        self.entries.last().ok_or(LedgerError::InternalError(
            "failed to retrieve entry after append",
        ))
    }

    /// Record resource regeneration (world to location).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_regeneration(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        world_entity: Uuid,
        location_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Regeneration,
            resource,
            quantity,
            from_entity: world_entity,
            from_entity_type: EntityType::World,
            to_entity: location_entity,
            to_entity_type: EntityType::Location,
            reason: "REGENERATION".to_owned(),
            reference_id: None,
        })
    }

    /// Record resource gathering (location to agent).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_gather(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        location_entity: Uuid,
        agent_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Gather,
            resource,
            quantity,
            from_entity: location_entity,
            from_entity_type: EntityType::Location,
            to_entity: agent_entity,
            to_entity_type: EntityType::Agent,
            reason: "GATHER".to_owned(),
            reference_id: None,
        })
    }

    /// Record resource consumption (agent to void).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_consumption(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        agent_entity: Uuid,
        void_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Consume,
            resource,
            quantity,
            from_entity: agent_entity,
            from_entity_type: EntityType::Agent,
            to_entity: void_entity,
            to_entity_type: EntityType::Void,
            reason: "CONSUME".to_owned(),
            reference_id: None,
        })
    }

    /// Record resource decay (structure to void).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_decay(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        structure_entity: Uuid,
        void_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Decay,
            resource,
            quantity,
            from_entity: structure_entity,
            from_entity_type: EntityType::Structure,
            to_entity: void_entity,
            to_entity_type: EntityType::Void,
            reason: "DECAY".to_owned(),
            reference_id: None,
        })
    }

    /// Record an agent-to-agent resource transfer (trade, gift).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_agent_transfer(
        &mut self,
        params: AgentTransferParams,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick: params.tick,
            entry_type: LedgerEntryType::Transfer,
            resource: params.resource,
            quantity: params.quantity,
            from_entity: params.from_agent,
            from_entity_type: EntityType::Agent,
            to_entity: params.to_agent,
            to_entity_type: EntityType::Agent,
            reason: params.reason,
            reference_id: params.reference_id,
        })
    }

    /// Record construction material usage (agent to structure).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_build(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        agent_entity: Uuid,
        structure_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Build,
            resource,
            quantity,
            from_entity: agent_entity,
            from_entity_type: EntityType::Agent,
            to_entity: structure_entity,
            to_entity_type: EntityType::Structure,
            reason: "BUILD".to_owned(),
            reference_id: Some(structure_entity),
        })
    }

    /// Record salvage material recovery (structure to agent).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_salvage(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        structure_entity: Uuid,
        agent_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Salvage,
            resource,
            quantity,
            from_entity: structure_entity,
            from_entity_type: EntityType::Structure,
            to_entity: agent_entity,
            to_entity_type: EntityType::Agent,
            reason: "SALVAGE".to_owned(),
            reference_id: Some(structure_entity),
        })
    }

    /// Record inventory drop on agent death (agent to location).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_drop(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        agent_entity: Uuid,
        location_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Drop,
            resource,
            quantity,
            from_entity: agent_entity,
            from_entity_type: EntityType::Agent,
            to_entity: location_entity,
            to_entity_type: EntityType::Location,
            reason: "DROP".to_owned(),
            reference_id: None,
        })
    }

    /// Record scavenging dropped items (location to agent).
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError`] if the entry fails validation.
    pub fn record_pickup(
        &mut self,
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        location_entity: Uuid,
        agent_entity: Uuid,
    ) -> Result<&LedgerEntry, LedgerError> {
        self.record_transfer(TransferParams {
            tick,
            entry_type: LedgerEntryType::Pickup,
            resource,
            quantity,
            from_entity: location_entity,
            from_entity_type: EntityType::Location,
            to_entity: agent_entity,
            to_entity_type: EntityType::Agent,
            reason: "PICKUP".to_owned(),
            reference_id: None,
        })
    }

    /// Verify the conservation law for a given tick.
    ///
    /// Returns [`ConservationResult::Balanced`] if the ledger is balanced,
    /// or [`ConservationResult::Anomaly`] with details about the imbalance.
    pub fn verify_conservation(&self, tick: u64) -> ConservationResult {
        verify_conservation(tick, &self.entries)
    }

    /// Verify the conservation law with strict flow semantics.
    ///
    /// This performs the basic double-entry balance check plus validates
    /// the flow direction semantics for each entry type.
    pub fn verify_conservation_strict(&self, tick: u64) -> ConservationResult {
        verify_conservation_strict(tick, &self.entries)
    }

    /// Return all entries for a given tick.
    pub fn entries_for_tick(&self, tick: u64) -> Vec<&LedgerEntry> {
        self.entries.iter().filter(|e| e.tick == tick).collect()
    }

    /// Return all entries, in insertion order.
    pub fn all_entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Calculate the net balance for a specific entity and resource.
    ///
    /// Positive balance means the entity has received more than it has sent.
    /// Negative balance means the entity has sent more than it has received.
    pub fn entity_balance(&self, entity_id: Uuid, resource: Resource) -> Decimal {
        let mut balance = Decimal::ZERO;

        for entry in &self.entries {
            if entry.resource != resource {
                continue;
            }

            // Credit: entity receives resources (is the `to_entity`).
            if entry.to_entity == Some(entity_id) {
                balance = balance.saturating_add(entry.quantity);
            }

            // Debit: entity loses resources (is the `from_entity`).
            if entry.from_entity == Some(entity_id) {
                balance = balance.saturating_sub(entry.quantity);
            }
        }

        balance
    }

    /// Calculate net resource flow for a specific tick.
    ///
    /// Returns a map of (resource, net change) for the given tick.
    /// Positive means net inflow (regeneration exceeds consumption and decay),
    /// negative means net outflow.
    pub fn net_flow_for_tick(&self, tick: u64) -> BTreeMap<Resource, Decimal> {
        let mut flows: BTreeMap<Resource, Decimal> = BTreeMap::new();

        for entry in &self.entries {
            if entry.tick != tick {
                continue;
            }

            match entry.entry_type {
                LedgerEntryType::Regeneration => {
                    let v = flows.entry(entry.resource).or_insert(Decimal::ZERO);
                    *v = v.saturating_add(entry.quantity);
                }
                LedgerEntryType::Consume | LedgerEntryType::Decay => {
                    let v = flows.entry(entry.resource).or_insert(Decimal::ZERO);
                    *v = v.saturating_sub(entry.quantity);
                }
                // Internal movements do not change the total resource count.
                LedgerEntryType::Gather
                | LedgerEntryType::Transfer
                | LedgerEntryType::Build
                | LedgerEntryType::Salvage
                | LedgerEntryType::Drop
                | LedgerEntryType::Pickup => {}
            }
        }

        flows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: create UUIDs for testing.
    fn id() -> Uuid {
        Uuid::now_v7()
    }

    /// Helper to build agent transfer params with common defaults.
    fn trade(
        tick: u64,
        resource: Resource,
        quantity: Decimal,
        from_agent: Uuid,
        to_agent: Uuid,
    ) -> AgentTransferParams {
        AgentTransferParams {
            tick,
            resource,
            quantity,
            from_agent,
            to_agent,
            reason: "TRADE".to_owned(),
            reference_id: None,
        }
    }

    #[test]
    fn new_ledger_is_empty() {
        let ledger = Ledger::new();
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
    }

    #[test]
    fn record_transfer_appends_entry() {
        let mut ledger = Ledger::new();
        let agent_a = id();
        let agent_b = id();

        let result =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(5, 0), agent_a, agent_b));

        assert!(result.is_ok());
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn transfer_between_agents_balances() {
        let mut ledger = Ledger::new();
        let agent_a = id();
        let agent_b = id();

        let _ =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(5, 0), agent_a, agent_b));

        let result = ledger.verify_conservation(1);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn regeneration_adds_to_world_total() {
        let mut ledger = Ledger::new();
        let world = id();
        let location = id();

        let _ =
            ledger.record_regeneration(1, Resource::Water, Decimal::new(20, 0), world, location);

        let flows = ledger.net_flow_for_tick(1);
        assert_eq!(
            flows.get(&Resource::Water).copied().unwrap_or(Decimal::ZERO),
            Decimal::new(20, 0),
        );

        let result = ledger.verify_conservation(1);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn consumption_removes_from_world_total() {
        let mut ledger = Ledger::new();
        let agent = id();
        let void = id();

        let _ = ledger.record_consumption(1, Resource::FoodBerry, Decimal::new(3, 0), agent, void);

        let flows = ledger.net_flow_for_tick(1);
        assert_eq!(
            flows
                .get(&Resource::FoodBerry)
                .copied()
                .unwrap_or(Decimal::ZERO),
            Decimal::new(-3, 0),
        );

        let result = ledger.verify_conservation(1);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn multi_resource_tick_balances() {
        let mut ledger = Ledger::new();
        let world = id();
        let location = id();
        let agent_a = id();
        let agent_b = id();
        let void = id();

        // Regenerate wood and water at location.
        let _ =
            ledger.record_regeneration(1, Resource::Wood, Decimal::new(10, 0), world, location);
        let _ =
            ledger.record_regeneration(1, Resource::Water, Decimal::new(20, 0), world, location);

        // Agent A gathers wood from location.
        let _ = ledger.record_gather(1, Resource::Wood, Decimal::new(8, 0), location, agent_a);

        // Agent A gathers water.
        let _ = ledger.record_gather(1, Resource::Water, Decimal::new(5, 0), location, agent_a);

        // Agent A trades wood to agent B.
        let _ =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(3, 0), agent_a, agent_b));

        // Agent B consumes water (they had some from a previous tick).
        let _ =
            ledger.record_consumption(1, Resource::Water, Decimal::new(2, 0), agent_b, void);

        let result = ledger.verify_conservation(1);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn entity_balance_tracks_correctly() {
        let mut ledger = Ledger::new();
        let agent_a = id();
        let agent_b = id();
        let location = id();

        // Agent A gathers 10 wood.
        let _ = ledger.record_gather(1, Resource::Wood, Decimal::new(10, 0), location, agent_a);

        // Agent A transfers 3 wood to agent B.
        let _ =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(3, 0), agent_a, agent_b));

        // Agent A should have net +7 wood.
        assert_eq!(
            ledger.entity_balance(agent_a, Resource::Wood),
            Decimal::new(7, 0),
        );

        // Agent B should have net +3 wood.
        assert_eq!(
            ledger.entity_balance(agent_b, Resource::Wood),
            Decimal::new(3, 0),
        );

        // Location should have net -10 wood.
        assert_eq!(
            ledger.entity_balance(location, Resource::Wood),
            Decimal::new(-10, 0),
        );
    }

    #[test]
    fn zero_quantity_rejected_via_ledger() {
        let mut ledger = Ledger::new();
        let result =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::ZERO, id(), id()));
        assert!(result.is_err());
        assert_eq!(ledger.len(), 0);
    }

    #[test]
    fn negative_quantity_rejected_via_ledger() {
        let mut ledger = Ledger::new();
        let result =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(-5, 0), id(), id()));
        assert!(result.is_err());
        assert_eq!(ledger.len(), 0);
    }

    #[test]
    fn entries_for_tick_filters_correctly() {
        let mut ledger = Ledger::new();
        let agent_a = id();
        let agent_b = id();

        let _ =
            ledger.record_agent_transfer(trade(1, Resource::Wood, Decimal::new(5, 0), agent_a, agent_b));
        let _ =
            ledger.record_agent_transfer(trade(2, Resource::Stone, Decimal::new(3, 0), agent_b, agent_a));

        assert_eq!(ledger.entries_for_tick(1).len(), 1);
        assert_eq!(ledger.entries_for_tick(2).len(), 1);
        assert_eq!(ledger.entries_for_tick(3).len(), 0);
    }

    #[test]
    fn build_and_salvage_cycle() {
        let mut ledger = Ledger::new();
        let agent = id();
        let structure = id();
        let void = id();

        // Agent builds structure with 20 wood.
        let _ = ledger.record_build(1, Resource::Wood, Decimal::new(20, 0), agent, structure);

        // Structure decays, losing 5 wood.
        let _ = ledger.record_decay(2, Resource::Wood, Decimal::new(5, 0), structure, void);

        // Agent salvages 6 wood from structure.
        let _ = ledger.record_salvage(3, Resource::Wood, Decimal::new(6, 0), structure, agent);

        // Each tick should be independently balanced.
        assert_eq!(ledger.verify_conservation(1), ConservationResult::Balanced);
        assert_eq!(ledger.verify_conservation(2), ConservationResult::Balanced);
        assert_eq!(ledger.verify_conservation(3), ConservationResult::Balanced);

        // Agent net: -20 (build) + 6 (salvage) = -14
        assert_eq!(
            ledger.entity_balance(agent, Resource::Wood),
            Decimal::new(-14, 0),
        );
    }

    #[test]
    fn drop_and_pickup_cycle() {
        let mut ledger = Ledger::new();
        let dying_agent = id();
        let location = id();
        let scavenger = id();

        // Dying agent drops 8 stone at location.
        let _ =
            ledger.record_drop(1, Resource::Stone, Decimal::new(8, 0), dying_agent, location);

        // Scavenger picks up 8 stone.
        let _ =
            ledger.record_pickup(1, Resource::Stone, Decimal::new(8, 0), location, scavenger);

        assert_eq!(ledger.verify_conservation(1), ConservationResult::Balanced);
    }

    #[test]
    fn strict_conservation_passes() {
        let mut ledger = Ledger::new();
        let world = id();
        let location = id();
        let agent = id();
        let void = id();

        let _ =
            ledger.record_regeneration(1, Resource::Wood, Decimal::new(10, 0), world, location);
        let _ = ledger.record_gather(1, Resource::Wood, Decimal::new(5, 0), location, agent);
        let _ = ledger.record_consumption(1, Resource::Wood, Decimal::new(2, 0), agent, void);

        let result = ledger.verify_conservation_strict(1);
        assert_eq!(result, ConservationResult::Balanced);
    }
}
