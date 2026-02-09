//! Error types for the emergence-agents crate.
//!
//! All operations that can fail return typed errors rather than panicking.
//! This module defines the error hierarchy used across agent state management,
//! vital mechanics, and inventory operations.

use emergence_types::{AgentId, Resource};

/// Errors that can occur during agent state operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Attempted to add resources that would exceed carry capacity.
    #[error("inventory overflow: adding {attempted} of {resource:?} would exceed capacity (current load: {current_load}, capacity: {capacity})")]
    InventoryOverflow {
        /// The resource type being added.
        resource: Resource,
        /// The quantity the caller attempted to add.
        attempted: u32,
        /// The agent's current total load.
        current_load: u32,
        /// The agent's maximum carry capacity.
        capacity: u32,
    },

    /// Attempted to remove more of a resource than the agent holds.
    #[error("insufficient resource: wanted {requested} of {resource:?} but only have {available}")]
    InsufficientResource {
        /// The resource type being removed.
        resource: Resource,
        /// The quantity the caller attempted to remove.
        requested: u32,
        /// The quantity the agent actually holds.
        available: u32,
    },

    /// An arithmetic overflow occurred during a vital computation.
    #[error("arithmetic overflow in vital computation: {context}")]
    ArithmeticOverflow {
        /// Description of what was being computed.
        context: String,
    },

    /// Agent with the given ID was not found in the manager.
    #[error("agent not found: {0}")]
    AgentNotFound(AgentId),

    /// Agent name already exists in the manager.
    #[error("duplicate agent name: {0}")]
    DuplicateName(String),

    /// Group formation failed validation.
    #[error("group formation failed: {reason}")]
    GroupFormationFailed {
        /// Description of why group formation was rejected.
        reason: String,
    },

    /// Reproduction failed a precondition check.
    #[error("reproduction failed: {reason}")]
    ReproductionFailed {
        /// Description of why reproduction was rejected.
        reason: String,
    },

    /// A governance action (claim, legislate, enforce) failed validation.
    #[error("governance action failed: {reason}")]
    GovernanceFailed {
        /// Description of why the governance action was rejected.
        reason: String,
    },
}
