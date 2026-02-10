//! Transaction builders and validation for the central ledger.
//!
//! Provides a [`TransactionBuilder`] that enforces the double-entry invariant:
//! every resource transfer must specify a source entity (debit) and a
//! destination entity (credit). Builders validate inputs before producing
//! a [`LedgerEntry`].

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use emergence_types::{EntityType, LedgerEntry, LedgerEntryId, LedgerEntryType, Resource};

use crate::LedgerError;

// ---------------------------------------------------------------------------
// Transaction builder
// ---------------------------------------------------------------------------

/// Builder for constructing validated [`LedgerEntry`] values.
///
/// Enforces that every entry has a valid entry type, a non-zero positive
/// quantity, and the correct source/destination entity types for the given
/// [`LedgerEntryType`].
///
/// # Examples
///
/// ```
/// use emergence_ledger::TransactionBuilder;
/// use emergence_types::{LedgerEntryType, Resource, EntityType};
/// use rust_decimal::Decimal;
/// use uuid::Uuid;
///
/// let entry = TransactionBuilder::new(1, LedgerEntryType::Gather, Resource::Wood)
///     .from(Uuid::now_v7(), EntityType::Location)
///     .to(Uuid::now_v7(), EntityType::Agent)
///     .quantity(Decimal::new(5, 0))
///     .reason("GATHER".to_owned())
///     .build();
///
/// assert!(entry.is_ok());
/// ```
#[derive(Debug)]
pub struct TransactionBuilder {
    tick: u64,
    entry_type: LedgerEntryType,
    resource: Resource,
    from_entity: Option<Uuid>,
    from_entity_type: Option<EntityType>,
    to_entity: Option<Uuid>,
    to_entity_type: Option<EntityType>,
    quantity: Option<Decimal>,
    reason: Option<String>,
    reference_id: Option<Uuid>,
}

impl TransactionBuilder {
    /// Start building a ledger entry for the given tick, entry type, and
    /// resource.
    pub const fn new(tick: u64, entry_type: LedgerEntryType, resource: Resource) -> Self {
        Self {
            tick,
            entry_type,
            resource,
            from_entity: None,
            from_entity_type: None,
            to_entity: None,
            to_entity_type: None,
            quantity: None,
            reason: None,
            reference_id: None,
        }
    }

    /// Set the source entity (debit side).
    #[must_use]
    pub const fn from(mut self, entity: Uuid, entity_type: EntityType) -> Self {
        self.from_entity = Some(entity);
        self.from_entity_type = Some(entity_type);
        self
    }

    /// Set the destination entity (credit side).
    #[must_use]
    pub const fn to(mut self, entity: Uuid, entity_type: EntityType) -> Self {
        self.to_entity = Some(entity);
        self.to_entity_type = Some(entity_type);
        self
    }

    /// Set the quantity of resource transferred.
    #[must_use]
    pub const fn quantity(mut self, qty: Decimal) -> Self {
        self.quantity = Some(qty);
        self
    }

    /// Set the human-readable reason for the transfer.
    #[must_use]
    pub fn reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Set an optional reference ID linking to a related entity.
    #[must_use]
    pub const fn reference_id(mut self, id: Uuid) -> Self {
        self.reference_id = Some(id);
        self
    }

    /// Validate inputs and produce a [`LedgerEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::ZeroQuantity`] if the quantity is zero.
    /// Returns [`LedgerError::NegativeQuantity`] if the quantity is negative.
    /// Returns [`LedgerError::MissingField`] if required fields are not set.
    /// Returns [`LedgerError::InvalidEntityType`] if the from/to entity types
    /// do not match the expected types for the entry type.
    pub fn build(self) -> Result<LedgerEntry, LedgerError> {
        let quantity = self.quantity.ok_or(LedgerError::MissingField("quantity"))?;
        let reason = self.reason.ok_or(LedgerError::MissingField("reason"))?;

        // Validate quantity is strictly positive.
        if quantity.is_zero() {
            return Err(LedgerError::ZeroQuantity);
        }
        if quantity.is_sign_negative() {
            return Err(LedgerError::NegativeQuantity { quantity });
        }

        // Validate entity types match the entry type contract.
        validate_entity_types(
            self.entry_type,
            self.from_entity_type,
            self.to_entity_type,
        )?;

        Ok(LedgerEntry {
            id: LedgerEntryId::new(),
            tick: self.tick,
            entry_type: self.entry_type,
            from_entity: self.from_entity,
            from_entity_type: self.from_entity_type,
            to_entity: self.to_entity,
            to_entity_type: self.to_entity_type,
            resource: self.resource,
            quantity,
            reason,
            reference_id: self.reference_id,
            created_at: Utc::now(),
        })
    }
}

/// Validate that the from/to entity types match the contract for the
/// given [`LedgerEntryType`].
fn validate_entity_types(
    entry_type: LedgerEntryType,
    from_type: Option<EntityType>,
    to_type: Option<EntityType>,
) -> Result<(), LedgerError> {
    let (expected_from, expected_to) = expected_entity_types(entry_type);

    if from_type != expected_from {
        return Err(LedgerError::InvalidEntityType {
            entry_type,
            side: "from",
            expected: format!("{expected_from:?}"),
            actual: format!("{from_type:?}"),
        });
    }

    if to_type != expected_to {
        return Err(LedgerError::InvalidEntityType {
            entry_type,
            side: "to",
            expected: format!("{expected_to:?}"),
            actual: format!("{to_type:?}"),
        });
    }

    Ok(())
}

/// Return the expected (from, to) entity types for each [`LedgerEntryType`].
const fn expected_entity_types(
    entry_type: LedgerEntryType,
) -> (Option<EntityType>, Option<EntityType>) {
    match entry_type {
        LedgerEntryType::Regeneration => (Some(EntityType::World), Some(EntityType::Location)),
        LedgerEntryType::Gather | LedgerEntryType::Pickup => {
            (Some(EntityType::Location), Some(EntityType::Agent))
        }
        LedgerEntryType::Consume => (Some(EntityType::Agent), Some(EntityType::Void)),
        LedgerEntryType::Transfer => (Some(EntityType::Agent), Some(EntityType::Agent)),
        LedgerEntryType::Build => (Some(EntityType::Agent), Some(EntityType::Structure)),
        LedgerEntryType::Salvage => (Some(EntityType::Structure), Some(EntityType::Agent)),
        LedgerEntryType::Decay => (Some(EntityType::Structure), Some(EntityType::Void)),
        LedgerEntryType::Drop => (Some(EntityType::Agent), Some(EntityType::Location)),
        LedgerEntryType::Theft | LedgerEntryType::CombatLoot => {
            (Some(EntityType::Agent), Some(EntityType::Agent))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_produces_valid_entry() {
        let from_id = Uuid::now_v7();
        let to_id = Uuid::now_v7();
        let result = TransactionBuilder::new(1, LedgerEntryType::Transfer, Resource::Wood)
            .from(from_id, EntityType::Agent)
            .to(to_id, EntityType::Agent)
            .quantity(Decimal::new(5, 0))
            .reason("TRADE".to_owned())
            .build();

        assert!(result.is_ok());
        let entry = result.ok();
        assert!(entry.is_some());
        if let Some(e) = entry {
            assert_eq!(e.tick, 1);
            assert_eq!(e.entry_type, LedgerEntryType::Transfer);
            assert_eq!(e.resource, Resource::Wood);
            assert_eq!(e.quantity, Decimal::new(5, 0));
        }
    }

    #[test]
    fn zero_quantity_rejected() {
        let result = TransactionBuilder::new(1, LedgerEntryType::Gather, Resource::Water)
            .from(Uuid::now_v7(), EntityType::Location)
            .to(Uuid::now_v7(), EntityType::Agent)
            .quantity(Decimal::ZERO)
            .reason("GATHER".to_owned())
            .build();

        assert!(result.is_err());
        let err = result.err();
        assert!(matches!(err, Some(LedgerError::ZeroQuantity)));
    }

    #[test]
    fn negative_quantity_rejected() {
        let result = TransactionBuilder::new(1, LedgerEntryType::Gather, Resource::Water)
            .from(Uuid::now_v7(), EntityType::Location)
            .to(Uuid::now_v7(), EntityType::Agent)
            .quantity(Decimal::new(-3, 0))
            .reason("GATHER".to_owned())
            .build();

        assert!(result.is_err());
        let err = result.err();
        assert!(matches!(err, Some(LedgerError::NegativeQuantity { .. })));
    }

    #[test]
    fn wrong_entity_type_rejected() {
        // Gather expects Location->Agent, not Agent->Agent
        let result = TransactionBuilder::new(1, LedgerEntryType::Gather, Resource::Wood)
            .from(Uuid::now_v7(), EntityType::Agent)
            .to(Uuid::now_v7(), EntityType::Agent)
            .quantity(Decimal::new(5, 0))
            .reason("GATHER".to_owned())
            .build();

        assert!(result.is_err());
        let err = result.err();
        assert!(matches!(err, Some(LedgerError::InvalidEntityType { .. })));
    }

    #[test]
    fn missing_quantity_rejected() {
        let result = TransactionBuilder::new(1, LedgerEntryType::Transfer, Resource::Wood)
            .from(Uuid::now_v7(), EntityType::Agent)
            .to(Uuid::now_v7(), EntityType::Agent)
            .reason("TRADE".to_owned())
            .build();

        assert!(result.is_err());
        let err = result.err();
        assert!(matches!(err, Some(LedgerError::MissingField("quantity"))));
    }

    #[test]
    fn reference_id_is_optional() {
        let ref_id = Uuid::now_v7();
        let result = TransactionBuilder::new(1, LedgerEntryType::Transfer, Resource::Stone)
            .from(Uuid::now_v7(), EntityType::Agent)
            .to(Uuid::now_v7(), EntityType::Agent)
            .quantity(Decimal::new(2, 0))
            .reason("TRADE".to_owned())
            .reference_id(ref_id)
            .build();

        assert!(result.is_ok());
        let entry = result.ok();
        assert!(entry.is_some());
        if let Some(e) = entry {
            assert_eq!(e.reference_id, Some(ref_id));
        }
    }

    #[test]
    fn all_entry_types_have_valid_entity_mappings() {
        // Ensure exhaustive coverage -- if a new entry type is added, this
        // test will fail to compile until its mapping is defined.
        let all_types = [
            LedgerEntryType::Regeneration,
            LedgerEntryType::Gather,
            LedgerEntryType::Consume,
            LedgerEntryType::Transfer,
            LedgerEntryType::Build,
            LedgerEntryType::Salvage,
            LedgerEntryType::Decay,
            LedgerEntryType::Drop,
            LedgerEntryType::Pickup,
        ];

        for entry_type in all_types {
            let (from, to) = expected_entity_types(entry_type);
            // Every entry type must have both a source and destination.
            assert!(
                from.is_some(),
                "entry type {entry_type:?} has no from entity type"
            );
            assert!(
                to.is_some(),
                "entry type {entry_type:?} has no to entity type"
            );
        }
    }
}
