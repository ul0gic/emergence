//! Agent state, vitals, actions, and lifecycle for the Emergence simulation.
//!
//! This crate contains the logic layer for agents -- everything that operates
//! on agent state without touching I/O. It sits between `emergence-types`
//! (which defines the data structures) and the engine/runner crates (which
//! handle persistence and orchestration).
//!
//! # Modules
//!
//! - [`actions`] -- Action validation, execution, conflict resolution, and costs.
//! - [`agent`] -- Agent creation and management ([`AgentManager`])
//! - [`config`] -- Configurable parameters for vital mechanics ([`VitalsConfig`])
//! - [`crafting`] -- Crafting recipes for workshop production (Tool, `ToolAdvanced`, Medicine)
//! - [`death`] -- Death conditions and consequences ([`DeathCause`], [`DeathConsequences`])
//! - [`deception`] -- Deception tracking, lie history, discovery mechanics
//! - [`diplomacy`] -- Diplomacy actions: alliances, conflicts, treaties, tribute
//! - [`error`] -- Error types for all agent operations ([`AgentError`])
//! - [`inventory`] -- Inventory (wallet) operations with carry capacity
//! - [`knowledge`] -- Knowledge base, tech tree, seed knowledge, discovery mechanics
//! - [`memory`] -- Tiered memory storage, compression, and perception filtering
//! - [`skills`] -- Skill levels, XP tracking, level-up mechanics, skill effects
//! - [`social`] -- Social graph, relationship tracking, and group formation
//! - [`reproduction`] -- Reproduction, maturity, aging, and population cap mechanics
//! - [`trade`] -- Trading system: offer, accept, reject, expiry, ledger integration
//! - [`vitals`] -- Per-tick vital mechanics (hunger, health, energy, aging)

pub mod actions;
pub mod agent;
pub mod config;
pub mod crafting;
pub mod death;
pub mod deception;
pub mod diplomacy;
pub mod error;
pub mod inventory;
pub mod knowledge;
pub mod memory;
pub mod reproduction;
pub mod skills;
pub mod social;
pub mod trade;
pub mod vitals;

// Re-export primary types at crate root for convenience.
pub use agent::{AgentManager, ChildAgentParams};
pub use config::VitalsConfig;
pub use death::{DeathCause, DeathConsequences};
pub use error::AgentError;
pub use knowledge::{
    DiscoveryConfig, DiscoveryMethod, KnowledgeBase, TechTree, attempt_discovery, attempt_teach,
    seed_knowledge,
};
pub use trade::{
    TradeAcceptResult, TradeError, DEFAULT_TRADE_EXPIRY_TICKS, expire_trade, is_trade_expired,
    trade_accept, trade_offer, trade_reject, validate_trade_offer_location,
    validate_trade_offer_resources,
};
pub use memory::{MemoryConfig, MemoryStore};
pub use skills::{
    MAX_SKILL_LEVEL, SKILL_NAMES, SkillSystem, XP_BUILD, XP_CRAFT, XP_FARM_HARVEST,
    XP_FARM_PLANT, XP_GATHER, XP_MINE, XP_MOVE, XP_SMELT, XP_TEACH, XP_TRADE,
};
pub use reproduction::{
    AgentBornDetails, ReproductionContext, blend_personality, can_add_agent,
    default_maturity_ticks, energy_cap, generate_child_name, immature_energy_cap,
    immature_gather_yield_pct, inherit_knowledge, is_action_restricted_for_immature, is_mature,
    movement_cost_multiplier, reproduction_energy_cost, validate_reproduction,
};
pub use crafting::{CraftRecipe, craftable_outputs, recipe_for};
pub use social::{SocialGraph, form_group};
pub use deception::{
    DeceptionDiscovery, DeceptionRecord, DeceptionSeverity, DeceptionTracker, DeceptionType,
    classify_severity,
};
pub use diplomacy::{
    Alliance, AllianceStatus, AllianceTerms, Conflict, DiplomacyError, DiplomacyResult,
    DiplomacyState, Treaty, TreatyTerms, TributeRecord,
};
pub use vitals::VitalTickResult;
