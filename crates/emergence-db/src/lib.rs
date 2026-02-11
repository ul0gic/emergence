//! Data layer for the Emergence simulation (`Dragonfly` + `PostgreSQL`).
//!
//! `Dragonfly` serves as the write-optimized hot state for the current tick.
//! `PostgreSQL` serves as the read-optimized cold state for full history and
//! analytics. This crate provides the interface to both stores and manages
//! the flush from hot to cold at the end of each tick.
//!
//! # Architecture (CQRS)
//!
//! ```text
//! Tick Execution
//!     |
//!     +-- Read/write hot state --> Dragonfly (DragonflyPool)
//!     |
//!     +-- End of tick flush ----> PostgreSQL (PostgresPool)
//!         |-- EventStore       (append-only events)
//!         |-- LedgerStore      (resource transfer records)
//!         +-- SnapshotStore    (world + agent snapshots)
//! ```
//!
//! # Modules
//!
//! - [`dragonfly`] -- `Dragonfly` (Redis-compatible) hot state operations
//! - [`postgres`] -- `PostgreSQL` connection pool and configuration
//! - [`event_store`] -- Batch event insertion and querying
//! - [`ledger_store`] -- Batch ledger entry insertion and querying
//! - [`snapshot_store`] -- World and agent snapshot persistence
//! - [`error`] -- Shared error types

pub mod dragonfly;
pub mod error;
pub mod event_store;
pub mod experiment_store;
pub mod ledger_store;
pub mod postgres;
pub mod snapshot_store;
pub mod tick_persist;

// Re-export primary types for convenience.
pub use dragonfly::DragonflyPool;
pub use error::DbError;
pub use event_store::{EventRow, EventStore};
pub use experiment_store::{ExperimentSnapshotRow, ExperimentStore};
pub use ledger_store::{LedgerRow, LedgerStore};
pub use postgres::{PostgresConfig, PostgresPool};
pub use snapshot_store::{AgentSnapshotRow, SnapshotStore, WorldSnapshotRow};
pub use tick_persist::PersistError;
