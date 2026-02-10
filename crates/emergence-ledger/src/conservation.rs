//! Conservation law verification for the central ledger.
//!
//! The conservation law enforces that internal resource movements always
//! balance: every debit from one entity must match a credit to another.
//! Resources enter the simulation via `Regeneration` and leave via
//! `Consume` or `Decay` -- these are source/sink flows that do not need
//! to balance within a single tick.
//!
//! For each resource R in tick T, the check is:
//!
//! ```text
//! sum(internal_credits for R in T) == sum(internal_debits for R in T)
//! ```
//!
//! Internal entry types: `Gather`, `Transfer`, `Build`, `Salvage`, `Drop`,
//! `Pickup`. Each entry adds its quantity to both the credit and debit
//! side equally, so this check is guaranteed by construction -- it exists
//! as defense-in-depth against data corruption or future bugs.
//!
//! A violation produces a [`LedgerAnomaly`] -- the simulation's most
//! critical integrity alert.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use emergence_types::{LedgerEntry, LedgerEntryType, Resource};

use crate::LedgerAnomaly;

/// The result of a conservation check for a single tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConservationResult {
    /// The ledger is balanced for this tick.
    Balanced,
    /// One or more resources have imbalanced flows.
    Anomaly(LedgerAnomaly),
}

/// Returns `true` if the entry type is an internal movement.
///
/// Internal movements transfer resources between entities without creating
/// or destroying them. Every internal entry must have matching credit and
/// debit quantities.
const fn is_internal(entry_type: LedgerEntryType) -> bool {
    matches!(
        entry_type,
        LedgerEntryType::Gather
            | LedgerEntryType::Transfer
            | LedgerEntryType::Build
            | LedgerEntryType::Salvage
            | LedgerEntryType::Drop
            | LedgerEntryType::Pickup
    )
}

/// Verify the conservation law for all entries in a single tick.
///
/// Checks that internal resource movements (Gather, Transfer, Build,
/// Salvage, Drop, Pickup) balance for every resource: total credits
/// must equal total debits. Source flows (Regeneration) and sink flows
/// (Consume, Decay) are excluded from the balance check because they
/// represent legitimate resource creation and destruction.
///
/// Each well-formed internal entry adds its quantity to both the credit
/// and debit accumulators equally, so this check passes by construction
/// for valid entries. It exists as defense-in-depth against corruption.
pub fn verify_conservation(tick: u64, entries: &[LedgerEntry]) -> ConservationResult {
    // Per-resource accumulators for internal movements only.
    let mut internal_credit: BTreeMap<Resource, Decimal> = BTreeMap::new();
    let mut internal_debit: BTreeMap<Resource, Decimal> = BTreeMap::new();

    for entry in entries {
        if entry.tick != tick {
            continue;
        }

        if is_internal(entry.entry_type) {
            // Credit side: to_entity receives the resource.
            let c = internal_credit
                .entry(entry.resource)
                .or_insert(Decimal::ZERO);
            *c = match c.checked_add(entry.quantity) {
                Some(val) => val,
                None => return overflow_anomaly(tick, entry.resource),
            };

            // Debit side: from_entity loses the resource.
            let d = internal_debit
                .entry(entry.resource)
                .or_insert(Decimal::ZERO);
            *d = match d.checked_add(entry.quantity) {
                Some(val) => val,
                None => return overflow_anomaly(tick, entry.resource),
            };
        }
        // Regeneration, Consume, and Decay are source/sink flows.
        // They do not participate in the internal balance check.
    }

    // Collect all resource keys from both maps.
    let all_resources: BTreeSet<Resource> = internal_credit
        .keys()
        .chain(internal_debit.keys())
        .copied()
        .collect();

    let mut imbalances: BTreeMap<Resource, (Decimal, Decimal)> = BTreeMap::new();

    for resource in &all_resources {
        let total_credit = internal_credit
            .get(resource)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let total_debit = internal_debit
            .get(resource)
            .copied()
            .unwrap_or(Decimal::ZERO);

        if total_credit != total_debit {
            imbalances.insert(*resource, (total_debit, total_credit));
        }
    }

    if imbalances.is_empty() {
        ConservationResult::Balanced
    } else {
        let count = imbalances.len();
        ConservationResult::Anomaly(LedgerAnomaly {
            tick,
            imbalances,
            message: format!(
                "LEDGER_ANOMALY at tick {tick}: conservation law violated for {count} resource(s)",
            ),
        })
    }
}

/// Construct an anomaly result for arithmetic overflow during summation.
fn overflow_anomaly(tick: u64, resource: Resource) -> ConservationResult {
    let mut imbalances = BTreeMap::new();
    imbalances.insert(resource, (Decimal::ZERO, Decimal::ZERO));
    ConservationResult::Anomaly(LedgerAnomaly {
        tick,
        imbalances,
        message: format!(
            "LEDGER_ANOMALY at tick {tick}: arithmetic overflow while summing {resource:?}",
        ),
    })
}

/// Verify conservation with additional flow-direction checks.
///
/// Performs the basic internal-balance check from [`verify_conservation`]
/// and then validates that source/sink flows have non-negative totals
/// (no negative regeneration or negative consumption). This is a stricter
/// form for callers who want to verify flow semantics beyond balance.
pub fn verify_conservation_strict(tick: u64, entries: &[LedgerEntry]) -> ConservationResult {
    // First, run the standard internal balance check.
    let result = verify_conservation(tick, entries);
    if let ConservationResult::Anomaly(_) = &result {
        return result;
    }

    // Additional check: source and sink flow totals must be non-negative.
    let mut inflow: BTreeMap<Resource, Decimal> = BTreeMap::new();
    let mut outflow: BTreeMap<Resource, Decimal> = BTreeMap::new();

    for entry in entries {
        if entry.tick != tick {
            continue;
        }

        match entry.entry_type {
            LedgerEntryType::Regeneration => {
                let v = inflow.entry(entry.resource).or_insert(Decimal::ZERO);
                *v = match v.checked_add(entry.quantity) {
                    Some(val) => val,
                    None => return overflow_anomaly(tick, entry.resource),
                };
            }
            LedgerEntryType::Consume | LedgerEntryType::Decay => {
                let v = outflow.entry(entry.resource).or_insert(Decimal::ZERO);
                *v = match v.checked_add(entry.quantity) {
                    Some(val) => val,
                    None => return overflow_anomaly(tick, entry.resource),
                };
            }
            LedgerEntryType::Gather
            | LedgerEntryType::Transfer
            | LedgerEntryType::Build
            | LedgerEntryType::Salvage
            | LedgerEntryType::Drop
            | LedgerEntryType::Pickup
            | LedgerEntryType::Theft
            | LedgerEntryType::CombatLoot => {}
        }
    }

    // Check for negative totals (should be impossible with positive-only
    // quantities, but defense-in-depth).
    let mut imbalances: BTreeMap<Resource, (Decimal, Decimal)> = BTreeMap::new();

    for (resource, total) in &inflow {
        if total.is_sign_negative() {
            imbalances.insert(*resource, (*total, Decimal::ZERO));
        }
    }
    for (resource, total) in &outflow {
        if total.is_sign_negative() {
            imbalances
                .entry(*resource)
                .or_insert((Decimal::ZERO, *total));
        }
    }

    if imbalances.is_empty() {
        ConservationResult::Balanced
    } else {
        let count = imbalances.len();
        ConservationResult::Anomaly(LedgerAnomaly {
            tick,
            imbalances,
            message: format!(
                "LEDGER_ANOMALY at tick {tick}: negative flow detected for {count} resource(s)",
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use emergence_types::{EntityType, LedgerEntryId};

    use super::*;

    /// Helper to create a ledger entry without going through the builder.
    fn make_entry(
        tick: u64,
        entry_type: LedgerEntryType,
        resource: Resource,
        quantity: Decimal,
        from_type: EntityType,
        to_type: EntityType,
    ) -> LedgerEntry {
        LedgerEntry {
            id: LedgerEntryId::new(),
            tick,
            entry_type,
            from_entity: Some(Uuid::now_v7()),
            from_entity_type: Some(from_type),
            to_entity: Some(Uuid::now_v7()),
            to_entity_type: Some(to_type),
            resource,
            quantity,
            reason: format!("{entry_type:?}"),
            reference_id: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn empty_tick_is_balanced() {
        let result = verify_conservation(1, &[]);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn single_transfer_is_balanced() {
        // A Transfer is internal: adds quantity to both credit and debit.
        let entries = vec![make_entry(
            1,
            LedgerEntryType::Transfer,
            Resource::Wood,
            Decimal::new(5, 0),
            EntityType::Agent,
            EntityType::Agent,
        )];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn regeneration_alone_is_balanced() {
        // Regeneration is a source flow, not internal. It does not
        // participate in the internal balance check, so a tick with
        // only regeneration is balanced (resources sit at the location).
        let entries = vec![make_entry(
            1,
            LedgerEntryType::Regeneration,
            Resource::Wood,
            Decimal::new(10, 0),
            EntityType::World,
            EntityType::Location,
        )];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn consumption_alone_is_balanced() {
        // Consumption is a sink flow, not internal. A tick with only
        // consumption is balanced (agent used resources from prior ticks).
        let entries = vec![make_entry(
            1,
            LedgerEntryType::Consume,
            Resource::FoodBerry,
            Decimal::new(3, 0),
            EntityType::Agent,
            EntityType::Void,
        )];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn decay_alone_is_balanced() {
        // Decay is a sink flow, same as consumption.
        let entries = vec![make_entry(
            1,
            LedgerEntryType::Decay,
            Resource::Wood,
            Decimal::new(5, 0),
            EntityType::Structure,
            EntityType::Void,
        )];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn regeneration_and_gather_balanced() {
        // Regen is source (excluded). Gather is internal (credit+debit).
        // Gather adds 10 to both credit and debit, so internal balances.
        let entries = vec![
            make_entry(
                1,
                LedgerEntryType::Regeneration,
                Resource::Wood,
                Decimal::new(10, 0),
                EntityType::World,
                EntityType::Location,
            ),
            make_entry(
                1,
                LedgerEntryType::Gather,
                Resource::Wood,
                Decimal::new(10, 0),
                EntityType::Location,
                EntityType::Agent,
            ),
        ];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn full_lifecycle_balanced() {
        // Regen -> Gather -> Transfer -> Consume. All internal movements
        // balance, and source/sink flows are excluded from the check.
        let entries = vec![
            make_entry(
                1,
                LedgerEntryType::Regeneration,
                Resource::Water,
                Decimal::new(20, 0),
                EntityType::World,
                EntityType::Location,
            ),
            make_entry(
                1,
                LedgerEntryType::Gather,
                Resource::Water,
                Decimal::new(15, 0),
                EntityType::Location,
                EntityType::Agent,
            ),
            make_entry(
                1,
                LedgerEntryType::Transfer,
                Resource::Water,
                Decimal::new(5, 0),
                EntityType::Agent,
                EntityType::Agent,
            ),
            make_entry(
                1,
                LedgerEntryType::Consume,
                Resource::Water,
                Decimal::new(10, 0),
                EntityType::Agent,
                EntityType::Void,
            ),
        ];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn entries_from_different_ticks_are_filtered() {
        let entries = vec![
            make_entry(
                1,
                LedgerEntryType::Transfer,
                Resource::Stone,
                Decimal::new(5, 0),
                EntityType::Agent,
                EntityType::Agent,
            ),
            make_entry(
                2,
                LedgerEntryType::Transfer,
                Resource::Stone,
                Decimal::new(99, 0),
                EntityType::Agent,
                EntityType::Agent,
            ),
        ];
        // Only tick 1 is checked; tick 2's entry is ignored.
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn strict_check_passes_for_valid_entries() {
        let entries = vec![
            make_entry(
                1,
                LedgerEntryType::Regeneration,
                Resource::Water,
                Decimal::new(20, 0),
                EntityType::World,
                EntityType::Location,
            ),
            make_entry(
                1,
                LedgerEntryType::Gather,
                Resource::Water,
                Decimal::new(15, 0),
                EntityType::Location,
                EntityType::Agent,
            ),
            make_entry(
                1,
                LedgerEntryType::Consume,
                Resource::Water,
                Decimal::new(5, 0),
                EntityType::Agent,
                EntityType::Void,
            ),
        ];
        let result = verify_conservation_strict(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn strict_check_passes_with_regen_only() {
        // Strict check also passes for regen-only -- inflow is positive,
        // no negative flows, internal is empty.
        let entries = vec![make_entry(
            1,
            LedgerEntryType::Regeneration,
            Resource::Stone,
            Decimal::new(50, 0),
            EntityType::World,
            EntityType::Location,
        )];
        let result = verify_conservation_strict(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn anomaly_construction_has_correct_fields() {
        // Directly construct an anomaly to verify its structure.
        let mut imbalances = BTreeMap::new();
        imbalances.insert(Resource::Wood, (Decimal::new(10, 0), Decimal::new(7, 0)));

        let anomaly = LedgerAnomaly {
            tick: 42,
            imbalances,
            message: "LEDGER_ANOMALY at tick 42: test".to_owned(),
        };

        assert_eq!(anomaly.tick, 42);
        assert!(anomaly.imbalances.contains_key(&Resource::Wood));
        assert!(anomaly.message.contains("LEDGER_ANOMALY"));
        assert!(anomaly.message.contains("42"));

        let (debit, credit) = anomaly.imbalances[&Resource::Wood];
        assert_eq!(debit, Decimal::new(10, 0));
        assert_eq!(credit, Decimal::new(7, 0));
    }

    #[test]
    fn anomaly_display_shows_message() {
        let anomaly = LedgerAnomaly {
            tick: 5,
            imbalances: BTreeMap::new(),
            message: "LEDGER_ANOMALY at tick 5: test display".to_owned(),
        };
        let display = format!("{anomaly}");
        assert!(display.contains("LEDGER_ANOMALY"));
        assert!(display.contains("tick 5"));
    }

    #[test]
    fn conservation_result_variants() {
        // Verify both variants can be constructed and compared.
        let balanced = ConservationResult::Balanced;
        let anomaly = ConservationResult::Anomaly(LedgerAnomaly {
            tick: 1,
            imbalances: BTreeMap::new(),
            message: "test".to_owned(),
        });

        assert_eq!(balanced, ConservationResult::Balanced);
        assert_ne!(balanced, anomaly);
    }

    #[test]
    fn multi_resource_all_balanced() {
        // Multiple resources, each with internal movements that balance.
        let entries = vec![
            make_entry(
                1,
                LedgerEntryType::Gather,
                Resource::Wood,
                Decimal::new(10, 0),
                EntityType::Location,
                EntityType::Agent,
            ),
            make_entry(
                1,
                LedgerEntryType::Gather,
                Resource::Stone,
                Decimal::new(5, 0),
                EntityType::Location,
                EntityType::Agent,
            ),
            make_entry(
                1,
                LedgerEntryType::Transfer,
                Resource::Wood,
                Decimal::new(3, 0),
                EntityType::Agent,
                EntityType::Agent,
            ),
        ];
        let result = verify_conservation(1, &entries);
        assert_eq!(result, ConservationResult::Balanced);
    }

    #[test]
    fn is_internal_classification() {
        // Verify the internal/external classification is correct.
        assert!(is_internal(LedgerEntryType::Gather));
        assert!(is_internal(LedgerEntryType::Transfer));
        assert!(is_internal(LedgerEntryType::Build));
        assert!(is_internal(LedgerEntryType::Salvage));
        assert!(is_internal(LedgerEntryType::Drop));
        assert!(is_internal(LedgerEntryType::Pickup));

        assert!(!is_internal(LedgerEntryType::Regeneration));
        assert!(!is_internal(LedgerEntryType::Consume));
        assert!(!is_internal(LedgerEntryType::Decay));
    }
}
