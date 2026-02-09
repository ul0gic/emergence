//! Central ledger and double-entry bookkeeping for the Emergence simulation.
//!
//! Every resource unit in the simulation is tracked through this ledger.
//! Resources are never created from nothing (except via regeneration) and never
//! destroyed into nothing (except via consumption or decay). The conservation
//! law is enforced at the end of every tick.
//!
//! # Architecture
//!
//! The ledger crate provides three modules:
//!
//! - [`ledger`] -- The [`Ledger`] struct: append-only log with recording methods.
//! - [`transaction`] -- The [`TransactionBuilder`] for validated entry construction.
//! - [`conservation`] -- Conservation law verification and anomaly detection.
//!
//! # Conservation Law
//!
//! For every tick T and every resource R:
//!
//! ```text
//! sum(credits for R in T) == sum(debits for R in T)
//! ```
//!
//! A violation produces a [`LedgerAnomaly`] -- the simulation's most critical
//! integrity alert. The ledger never panics; it returns errors.
//!
//! # Double-Entry Bookkeeping
//!
//! Every resource transfer records both sides:
//! - **Debit**: the source entity loses the resource quantity.
//! - **Credit**: the destination entity gains the resource quantity.
//!
//! Entry types and their expected entity pairs:
//!
//! | Type | From (debit) | To (credit) |
//! |------|-------------|-------------|
//! | Regeneration | World | Location |
//! | Gather | Location | Agent |
//! | Consume | Agent | Void |
//! | Transfer | Agent | Agent |
//! | Build | Agent | Structure |
//! | Salvage | Structure | Agent |
//! | Decay | Structure | Void |
//! | Drop | Agent | Location |
//! | Pickup | Location | Agent |
//!
//! # Usage
//!
//! ```
//! use emergence_ledger::{Ledger, TransactionBuilder};
//! use emergence_ledger::conservation::ConservationResult;
//! use emergence_types::{LedgerEntryType, Resource, EntityType};
//! use rust_decimal::Decimal;
//! use uuid::Uuid;
//!
//! let mut ledger = Ledger::new();
//! let world = Uuid::now_v7();
//! let location = Uuid::now_v7();
//! let agent = Uuid::now_v7();
//!
//! // World regenerates 10 wood at location.
//! ledger.record_regeneration(1, Resource::Wood, Decimal::new(10, 0), world, location)
//!     .ok();
//!
//! // Agent gathers 5 wood from location.
//! ledger.record_gather(1, Resource::Wood, Decimal::new(5, 0), location, agent)
//!     .ok();
//!
//! // Verify conservation law holds.
//! assert_eq!(ledger.verify_conservation(1), ConservationResult::Balanced);
//! ```

pub mod conservation;
pub mod ledger;
pub mod transaction;

// Re-export primary types at crate root.
pub use conservation::ConservationResult;
pub use ledger::{AgentTransferParams, Ledger, TransferParams};
pub use transaction::TransactionBuilder;

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use emergence_types::{LedgerEntryType, Resource};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when recording ledger entries.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    /// Quantity must be strictly positive.
    #[error("ledger entry quantity must be non-zero")]
    ZeroQuantity,

    /// Quantity must not be negative.
    #[error("ledger entry quantity must be positive, got {quantity}")]
    NegativeQuantity {
        /// The invalid quantity.
        quantity: Decimal,
    },

    /// A required field was not set on the builder.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// The from/to entity types do not match the expected types for the
    /// entry type.
    #[error(
        "invalid entity type for {entry_type:?} {side}: expected {expected}, got {actual}"
    )]
    InvalidEntityType {
        /// The entry type being validated.
        entry_type: LedgerEntryType,
        /// Which side of the entry ("from" or "to").
        side: &'static str,
        /// The expected entity type.
        expected: String,
        /// The actual entity type.
        actual: String,
    },

    /// An internal error that should not occur in normal operation.
    #[error("internal ledger error: {0}")]
    InternalError(&'static str),
}

// ---------------------------------------------------------------------------
// Anomaly type
// ---------------------------------------------------------------------------

/// A conservation law violation detected during tick verification.
///
/// This is the `LEDGER_ANOMALY` alert described in the spec. When the
/// conservation check finds that credits and debits do not balance for
/// one or more resources in a tick, this struct captures the details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerAnomaly {
    /// The tick where the anomaly was detected.
    pub tick: u64,
    /// Per-resource imbalance: (`debit_total`, `credit_total`) for each
    /// resource that did not balance.
    pub imbalances: BTreeMap<Resource, (Decimal, Decimal)>,
    /// Human-readable description of the anomaly.
    pub message: String,
}

impl core::fmt::Display for LedgerAnomaly {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}
