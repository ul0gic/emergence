//! Event sourcing and state snapshots for the Emergence simulation.
//!
//! Every state change produces an immutable event written to the event store.
//! Events are the source of truth -- state can be reconstructed by replaying
//! them. This crate defines event types, the event store interface, and
//! periodic state snapshot logic.
