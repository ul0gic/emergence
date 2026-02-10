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
//! - [`operator`] -- Shared operator control state for pause, resume,
//!   speed adjustment, event injection, and clean shutdown.
//! - [`perception`] -- Per-agent perception assembly from world state.
//! - [`runner`] -- Top-level simulation loop with operator controls,
//!   boundary enforcement, and clean shutdown sequencing.
//! - [`tick`] -- The 6-phase tick cycle engine loop.
//!
//! [`DecisionSource`]: decision::DecisionSource
//! [`StubDecisionSource`]: decision::StubDecisionSource

pub mod clock;
pub mod config;
pub mod decision;
pub mod feasibility;
pub mod fuzzy;
pub mod operator;
pub mod perception;
pub mod runner;
pub mod tick;
