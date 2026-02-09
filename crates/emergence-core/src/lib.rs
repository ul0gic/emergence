//! World clock, tick cycle, and orchestration for the Emergence simulation.
//!
//! This crate owns the 6-phase tick cycle that drives the simulation:
//! World Wake, Perception, Decision, Resolution, Persist, and Reflection.
//!
//! # Modules
//!
//! - [`clock`] -- World clock with tick counter, era tracking, season
//!   derivation, and time-of-day computation.
//! - [`config`] -- Configuration loading from `emergence-config.yaml` into
//!   strongly-typed structs.
//! - [`decision`] -- [`DecisionSource`] trait and [`StubDecisionSource`].
//! - [`fuzzy`] -- Fuzzy resource quantity representation for perception.
//! - [`perception`] -- Per-agent perception assembly from world state.
//! - [`tick`] -- The 6-phase tick cycle engine loop.
//!
//! [`DecisionSource`]: decision::DecisionSource
//! [`StubDecisionSource`]: decision::StubDecisionSource

pub mod clock;
pub mod config;
pub mod decision;
pub mod fuzzy;
pub mod perception;
pub mod tick;
