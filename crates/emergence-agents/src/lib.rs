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
//! - [`belief_detection`] -- Belief/narrative detection from agent communications
//! - [`communication`] -- Private/secret communication: whisper, conspire, location announcements
//! - [`config`] -- Configurable parameters for vital mechanics ([`VitalsConfig`])
//! - [`constructs`] -- Social construct data model (religion, governance, etc.)
//! - [`crafting`] -- Crafting recipes for workshop production (Tool, `ToolAdvanced`, Medicine)
//! - [`crime_justice`] -- Crime recording, punishment tracking, justice system classification
//! - [`death`] -- Death conditions and consequences ([`DeathCause`], [`DeathConsequences`])
//! - [`deception`] -- Deception tracking, lie history, discovery mechanics
//! - [`diplomacy`] -- Diplomacy actions: alliances, conflicts, treaties, tribute
//! - [`economy_detection`] -- Economic system detection, currency, markets, Gini coefficient
//! - [`error`] -- Error types for all agent operations ([`AgentError`])
//! - [`family`] -- Family units, lineage tracking, marriage/divorce/birth records
//! - [`governance`] -- Governance structure tracking and classification
//! - [`inventory`] -- Inventory (wallet) operations with carry capacity
//! - [`knowledge`] -- Knowledge base, tech tree, seed knowledge, discovery mechanics
//! - [`memory`] -- Tiered memory storage, compression, and perception filtering
//! - [`persuasion`] -- Persuasion mechanics: belief change, recruitment, allegiance shifts
//! - [`propaganda`] -- Persistent public declarations at locations that influence newcomers
//! - [`reputation`] -- Observable reputation system: tags, observations, decay, perception summaries
//! - [`skills`] -- Skill levels, XP tracking, level-up mechanics, skill effects
//! - [`social`] -- Social graph, relationship tracking, and group formation
//! - [`reproduction`] -- Reproduction, maturity, aging, and population cap mechanics
//! - [`trade`] -- Trading system: offer, accept, reject, expiry, ledger integration
//! - [`vitals`] -- Per-tick vital mechanics (hunger, health, energy, aging)

pub mod actions;
pub mod agent;
pub mod belief_detection;
pub mod communication;
pub mod config;
pub mod constructs;
pub mod crafting;
pub mod crime_justice;
pub mod death;
pub mod deception;
pub mod diplomacy;
pub mod economy_detection;
pub mod error;
pub mod family;
pub mod governance;
pub mod inventory;
pub mod knowledge;
pub mod memory;
pub mod persuasion;
pub mod propaganda;
pub mod reputation;
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
pub use constructs::{
    ConstructEvent, ConstructEventType, ConstructRegistry, SocialConstruct,
    SocialConstructCategory,
};
pub use belief_detection::{BeliefDetector, BeliefTheme, DetectedBelief, SchismRisk};
pub use governance::{
    GovernanceTracker, GovernanceType, LeadershipClaim, RuleDeclaration, VoteRecord,
};
pub use family::{FamilyBond, FamilyRole, FamilyTracker, FamilyUnit};
pub use economy_detection::{
    EconomicDetector, EconomicEvent, EconomicIndicator, EconomicModel,
};
pub use crime_justice::{
    CrimeRecord, CrimeTracker, CrimeType, JusticePattern, PunishmentRecord, PunishmentType,
};
pub use communication::{
    CommunicationStats, MessageRouter, MessageVisibility, PrivateMessage,
};
pub use persuasion::{
    PersuasionAttempt, PersuasionContext, PersuasionEvaluator, PersuasionRecord, PersuasionResult,
    PersuasionType,
};
pub use reputation::{
    ActionReputationEvent, ReputationAction, ReputationEntry, ReputationObservation,
    ReputationProfile, ReputationTag, ReputationTracker,
};
pub use propaganda::{
    PropagandaBoard, PropagandaInfluence, PropagandaParams, PropagandaPost, PropagandaType,
};
