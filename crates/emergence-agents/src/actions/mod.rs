//! Action validation, execution, and conflict resolution for agent actions.
//!
//! This module implements the full action pipeline from `world-engine.md`
//! section 7: syntax validation, vitals checks, location validation,
//! resource verification, world state checks, skill requirements,
//! conflict resolution, and execution.
//!
//! # Submodules
//!
//! - [`costs`] -- Energy costs and food values per action type.
//! - [`handlers`] -- Execution logic for each survival action.
//! - [`validation`] -- The 7-stage validation pipeline.
//! - [`conflict`] -- Conflict resolution for contested resources.

pub mod conflict;
pub mod costs;
pub mod handlers;
pub mod validation;
