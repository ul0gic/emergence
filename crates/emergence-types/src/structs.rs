//! Core entity structs for the Emergence simulation.
//!
//! Covers `Personality`, `MemoryEntry`, `ResourceNode`, `AccessControlList`,
//! `StructureProperties`, and snapshot/context types from `data-schemas.md`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::enums::{
    Era, EventType, LedgerEntryType, MemoryTier, Resource, Season, StructureType, Weather,
};
use crate::ids::{
    AgentId, EventId, GroupId, LedgerEntryId, LocationId, RouteId, RuleId, StructureId, TradeId,
};

// ---------------------------------------------------------------------------
// 4.2 Personality
// ---------------------------------------------------------------------------

/// Immutable personality vector assigned at agent creation.
///
/// Each trait is a [`Decimal`] in the range 0.0 to 1.0. Personality influences
/// decision-making but never changes over the agent's lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Personality {
    /// Likelihood to explore, try new things, and learn from observation.
    #[ts(as = "String")]
    pub curiosity: Decimal,
    /// Preference for collaboration versus solo action.
    #[ts(as = "String")]
    pub cooperation: Decimal,
    /// Tendency toward conflict, competition, and dominance.
    #[ts(as = "String")]
    pub aggression: Decimal,
    /// Willingness to take uncertain actions.
    #[ts(as = "String")]
    pub risk_tolerance: Decimal,
    /// Preference for productive work versus rest or leisure.
    #[ts(as = "String")]
    pub industriousness: Decimal,
    /// Desire for interaction versus solitude.
    #[ts(as = "String")]
    pub sociability: Decimal,
    /// Tendency toward truthful communication.
    #[ts(as = "String")]
    pub honesty: Decimal,
    /// Commitment to relationships and groups.
    #[ts(as = "String")]
    pub loyalty: Decimal,
}

// ---------------------------------------------------------------------------
// 4.10 MemoryEntry
// ---------------------------------------------------------------------------

/// A single memory stored by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct MemoryEntry {
    /// The tick when the memory was formed.
    pub tick: u64,
    /// Category of the memory (action, observation, communication, etc.).
    pub memory_type: String,
    /// Human-readable description of what happened.
    pub summary: String,
    /// Related entity identifiers.
    pub entities: Vec<Uuid>,
    /// Retention importance (0.0 to 1.0). Higher means more likely retained.
    #[ts(as = "String")]
    pub emotional_weight: Decimal,
    /// Which retention tier this memory belongs to.
    pub tier: MemoryTier,
}

/// Well-known memory type category constants.
///
/// These correspond to the `memory_type` field values defined in
/// `data-schemas.md` section 4.10.
pub mod memory_types {
    /// An action the agent performed.
    pub const ACTION: &str = "action";
    /// Something the agent observed in the world.
    pub const OBSERVATION: &str = "observation";
    /// A communication event (message sent or received).
    pub const COMMUNICATION: &str = "communication";
    /// A knowledge discovery.
    pub const DISCOVERY: &str = "discovery";
    /// A social event (relationship change, group formation, death notification).
    pub const SOCIAL: &str = "social";
}

impl MemoryEntry {
    /// Create a new memory entry for an action the agent performed.
    ///
    /// `emotional_weight` is clamped to the 0.0--1.0 range.
    /// New entries always start in the [`MemoryTier::Immediate`] tier.
    pub fn action(
        tick: u64,
        summary: String,
        entities: Vec<Uuid>,
        emotional_weight: Decimal,
    ) -> Self {
        Self {
            tick,
            memory_type: String::from(memory_types::ACTION),
            summary,
            entities,
            emotional_weight: clamp_weight(emotional_weight),
            tier: MemoryTier::Immediate,
        }
    }

    /// Create a new memory entry for an observation.
    ///
    /// `emotional_weight` is clamped to the 0.0--1.0 range.
    /// New entries always start in the [`MemoryTier::Immediate`] tier.
    pub fn observation(
        tick: u64,
        summary: String,
        entities: Vec<Uuid>,
        emotional_weight: Decimal,
    ) -> Self {
        Self {
            tick,
            memory_type: String::from(memory_types::OBSERVATION),
            summary,
            entities,
            emotional_weight: clamp_weight(emotional_weight),
            tier: MemoryTier::Immediate,
        }
    }

    /// Create a new memory entry for a communication event.
    ///
    /// `emotional_weight` is clamped to the 0.0--1.0 range.
    /// New entries always start in the [`MemoryTier::Immediate`] tier.
    pub fn communication(
        tick: u64,
        summary: String,
        entities: Vec<Uuid>,
        emotional_weight: Decimal,
    ) -> Self {
        Self {
            tick,
            memory_type: String::from(memory_types::COMMUNICATION),
            summary,
            entities,
            emotional_weight: clamp_weight(emotional_weight),
            tier: MemoryTier::Immediate,
        }
    }

    /// Create a new memory entry for a knowledge discovery.
    ///
    /// `emotional_weight` is clamped to the 0.0--1.0 range.
    /// New entries always start in the [`MemoryTier::Immediate`] tier.
    pub fn discovery(
        tick: u64,
        summary: String,
        entities: Vec<Uuid>,
        emotional_weight: Decimal,
    ) -> Self {
        Self {
            tick,
            memory_type: String::from(memory_types::DISCOVERY),
            summary,
            entities,
            emotional_weight: clamp_weight(emotional_weight),
            tier: MemoryTier::Immediate,
        }
    }

    /// Create a new memory entry for a social event.
    ///
    /// `emotional_weight` is clamped to the 0.0--1.0 range.
    /// New entries always start in the [`MemoryTier::Immediate`] tier.
    pub fn social(
        tick: u64,
        summary: String,
        entities: Vec<Uuid>,
        emotional_weight: Decimal,
    ) -> Self {
        Self {
            tick,
            memory_type: String::from(memory_types::SOCIAL),
            summary,
            entities,
            emotional_weight: clamp_weight(emotional_weight),
            tier: MemoryTier::Immediate,
        }
    }

    /// Check whether this memory references a specific entity (agent, location, etc.).
    pub fn involves_entity(&self, entity_id: Uuid) -> bool {
        self.entities.contains(&entity_id)
    }

    /// Check whether this memory references any of the given entities.
    pub fn involves_any_entity(&self, entity_ids: &[Uuid]) -> bool {
        entity_ids.iter().any(|id| self.entities.contains(id))
    }

    /// Check whether the summary contains a keyword (case-insensitive).
    pub fn matches_topic(&self, keyword: &str) -> bool {
        let lower_summary = self.summary.to_lowercase();
        let lower_keyword = keyword.to_lowercase();
        lower_summary.contains(&lower_keyword)
    }

    /// Approximate token count for this memory entry.
    ///
    /// Uses a rough heuristic of 1 token per 4 characters of the summary text.
    /// This is a fast approximation, not a true tokenizer.
    pub const fn approximate_tokens(&self) -> usize {
        let char_count = self.summary.len();
        if char_count == 0 {
            return 0;
        }
        // Integer division, minimum 1 for non-empty summaries
        let tokens = char_count / 4;
        if tokens == 0 { 1 } else { tokens }
    }
}

/// Clamp a [`Decimal`] weight to the 0.0--1.0 range.
fn clamp_weight(weight: Decimal) -> Decimal {
    if weight < Decimal::ZERO {
        Decimal::ZERO
    } else if weight > Decimal::ONE {
        Decimal::ONE
    } else {
        weight
    }
}

// ---------------------------------------------------------------------------
// 4.5 ResourceNode
// ---------------------------------------------------------------------------

/// A resource source at a location with regeneration mechanics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ResourceNode {
    /// The type of resource at this node.
    pub resource: Resource,
    /// Currently available quantity.
    pub available: u32,
    /// Units regenerated per tick.
    pub regen_per_tick: u32,
    /// Maximum quantity this node can hold.
    pub max_capacity: u32,
}

// ---------------------------------------------------------------------------
// 4.7 AccessControlList
// ---------------------------------------------------------------------------

/// Access restrictions for routes and structures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AccessControlList {
    /// Agents explicitly allowed access.
    pub allowed_agents: BTreeSet<AgentId>,
    /// Groups explicitly allowed access.
    pub allowed_groups: BTreeSet<GroupId>,
    /// Agents explicitly denied access.
    pub denied_agents: BTreeSet<AgentId>,
    /// If true, open to all agents regardless of allow/deny lists.
    pub public: bool,
    /// Optional toll cost required for passage.
    pub toll_cost: Option<BTreeMap<Resource, u32>>,
}

// ---------------------------------------------------------------------------
// 4.9 StructureProperties
// ---------------------------------------------------------------------------

/// Type-specific properties attached to a structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureProperties {
    /// Multiplier for rest recovery when sheltering in this structure.
    #[ts(as = "String")]
    pub rest_bonus: Decimal,
    /// Whether this structure blocks weather effects.
    pub weather_protection: bool,
    /// Additional inventory storage slots provided.
    pub storage_slots: u32,
    /// The resource this structure produces, if any.
    pub production_type: Option<Resource>,
    /// Units produced per tick.
    pub production_rate: u32,
}

// ---------------------------------------------------------------------------
// 5.2 WorldContext
// ---------------------------------------------------------------------------

/// Snapshot of world-level state attached to events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WorldContext {
    /// Current tick number.
    pub tick: u64,
    /// Current civilizational era.
    pub era: Era,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
    /// Number of living agents.
    pub population: u32,
}

// ---------------------------------------------------------------------------
// 5.3 AgentStateSnapshot
// ---------------------------------------------------------------------------

/// Lightweight snapshot of agent state captured at event time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AgentStateSnapshot {
    /// Energy at event time.
    pub energy: u32,
    /// Health at event time.
    pub health: u32,
    /// Hunger at event time.
    pub hunger: u32,
    /// Age in ticks at event time.
    pub age: u32,
    /// Location at event time.
    pub location_id: LocationId,
    /// Inventory summary at event time.
    pub inventory_summary: BTreeMap<Resource, u32>,
}

// ---------------------------------------------------------------------------
// 5.1 Base Event
// ---------------------------------------------------------------------------

/// An immutable event recorded in the event store.
///
/// Events are the source of truth for the simulation's history. State can
/// be reconstructed by replaying the event log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Event {
    /// Unique event identifier.
    pub id: EventId,
    /// The tick when this event occurred.
    pub tick: u64,
    /// The category of event.
    pub event_type: EventType,
    /// The primary agent involved, if any.
    pub agent_id: Option<AgentId>,
    /// The location where the event occurred, if applicable.
    pub location_id: Option<LocationId>,
    /// Type-specific payload serialized as JSON.
    pub details: serde_json::Value,
    /// Agent state at the time of the event, if applicable.
    pub agent_state_snapshot: Option<AgentStateSnapshot>,
    /// World-level context at the time of the event.
    pub world_context: WorldContext,
    /// Real-world timestamp when the event was created.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// 6.1 LedgerEntry
// ---------------------------------------------------------------------------

/// A single entry in the central ledger tracking all resource transfers.
///
/// Every resource movement in the simulation produces a ledger entry.
/// The ledger must balance at the end of every tick (conservation law).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct LedgerEntry {
    /// Unique entry identifier.
    pub id: LedgerEntryId,
    /// The tick when this transfer occurred.
    pub tick: u64,
    /// The category of transfer.
    pub entry_type: LedgerEntryType,
    /// Source entity, if any (`None` for world-sourced regeneration).
    pub from_entity: Option<Uuid>,
    /// Type of the source entity.
    pub from_entity_type: Option<crate::enums::EntityType>,
    /// Destination entity, if any (`None` for void-consumed resources).
    pub to_entity: Option<Uuid>,
    /// Type of the destination entity.
    pub to_entity_type: Option<crate::enums::EntityType>,
    /// The resource being transferred.
    pub resource: Resource,
    /// Quantity transferred (always positive; uses [`Decimal`] for financial-grade precision).
    #[ts(as = "String")]
    pub quantity: Decimal,
    /// Reason for the transfer (e.g. `"GATHER"`, `"TRADE"`, `"BUILD"`).
    pub reason: String,
    /// Related entity such as a trade or structure ID.
    pub reference_id: Option<Uuid>,
    /// Real-world timestamp.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// 4.1 Agent (identity)
// ---------------------------------------------------------------------------

/// Immutable agent identity established at creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Agent {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Display name, unique within the simulation.
    pub name: String,
    /// Tick when the agent entered the simulation.
    pub born_at_tick: u64,
    /// Tick when the agent died (`None` if alive).
    pub died_at_tick: Option<u64>,
    /// How the agent died, if applicable.
    pub cause_of_death: Option<String>,
    /// First parent (if reproduced; `None` for seed agents).
    pub parent_a: Option<AgentId>,
    /// Second parent (if reproduced; `None` for seed agents).
    pub parent_b: Option<AgentId>,
    /// Generation number (0 for seed agents).
    pub generation: u32,
    /// Immutable personality vector.
    pub personality: Personality,
    /// Real-world creation time.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// 4.3 AgentState (mutable)
// ---------------------------------------------------------------------------

/// Mutable state of an agent that changes each tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AgentState {
    /// Reference to the agent this state belongs to.
    pub agent_id: AgentId,
    /// Current energy (0--100).
    pub energy: u32,
    /// Current health (0--100).
    pub health: u32,
    /// Current hunger level (0--100).
    pub hunger: u32,
    /// Current age in ticks.
    pub age: u32,
    /// The tick when the agent entered the simulation.
    ///
    /// Seed agents have `born_at_tick = 0`. Child agents born via
    /// reproduction have `born_at_tick > 0`. Used for maturity checks.
    pub born_at_tick: u64,
    /// Current location.
    pub location_id: LocationId,
    /// Travel destination, if in transit.
    pub destination_id: Option<LocationId>,
    /// Ticks remaining until arrival (0 if not traveling).
    pub travel_progress: u32,
    /// Carried resources.
    pub inventory: BTreeMap<Resource, u32>,
    /// Maximum carry weight.
    pub carry_capacity: u32,
    /// Set of known concepts (knowledge base).
    pub knowledge: BTreeSet<String>,
    /// Skill name to level mapping.
    pub skills: BTreeMap<String, u32>,
    /// Skill name to experience points mapping.
    pub skill_xp: BTreeMap<String, u32>,
    /// Active goals (max 5).
    pub goals: Vec<String>,
    /// Social graph: agent ID to relationship score.
    #[ts(as = "BTreeMap<AgentId, String>")]
    pub relationships: BTreeMap<AgentId, Decimal>,
    /// Agent's memory entries.
    pub memory: Vec<MemoryEntry>,
}

// ---------------------------------------------------------------------------
// 4.4 Location
// ---------------------------------------------------------------------------

/// A location node in the world graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Location {
    /// Unique location identifier.
    pub id: LocationId,
    /// Display name.
    pub name: String,
    /// Region this location belongs to.
    pub region: String,
    /// Category (natural, settlement, etc.).
    pub location_type: String,
    /// Narrative description.
    pub description: String,
    /// Maximum number of agents.
    pub capacity: u32,
    /// Resource availability at this location.
    pub base_resources: BTreeMap<Resource, ResourceNode>,
    /// Agents who know about this location.
    pub discovered_by: BTreeSet<AgentId>,
    /// Real-world creation time.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// 4.6 Route
// ---------------------------------------------------------------------------

/// A directed edge connecting two locations in the world graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Route {
    /// Unique route identifier.
    pub id: RouteId,
    /// Origin location.
    pub from_location: LocationId,
    /// Destination location.
    pub to_location: LocationId,
    /// Travel time in ticks.
    pub cost_ticks: u32,
    /// Road quality.
    pub path_type: crate::enums::PathType,
    /// Current condition (0--100).
    pub durability: u32,
    /// Maximum condition.
    pub max_durability: u32,
    /// Degradation rate per tick.
    #[ts(as = "String")]
    pub decay_per_tick: Decimal,
    /// Access restrictions, if any.
    pub acl: Option<AccessControlList>,
    /// Whether the route works in both directions.
    pub bidirectional: bool,
    /// Agent who built this route (`None` if natural).
    pub built_by: Option<AgentId>,
    /// Tick when the route was built (`None` if natural).
    pub built_at_tick: Option<u64>,
}

// ---------------------------------------------------------------------------
// 4.8 Structure
// ---------------------------------------------------------------------------

/// A persistent structure built by an agent at a location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Structure {
    /// Unique structure identifier.
    pub id: StructureId,
    /// The type of structure.
    pub structure_type: StructureType,
    /// Specific variant within the type.
    pub subtype: Option<String>,
    /// Location where this structure exists.
    pub location_id: LocationId,
    /// Agent who built this structure.
    pub builder: AgentId,
    /// Current owner (`None` if unowned).
    pub owner: Option<AgentId>,
    /// Tick when the structure was built.
    pub built_at_tick: u64,
    /// Tick when the structure was destroyed (`None` if standing).
    pub destroyed_at_tick: Option<u64>,
    /// Materials used to construct this structure.
    pub materials_used: BTreeMap<Resource, u32>,
    /// Current condition (0--100).
    pub durability: u32,
    /// Maximum condition.
    pub max_durability: u32,
    /// Degradation rate per tick.
    #[ts(as = "String")]
    pub decay_per_tick: Decimal,
    /// Maximum occupant count.
    pub capacity: u32,
    /// Current occupants.
    pub occupants: BTreeSet<AgentId>,
    /// Access restrictions, if any.
    pub access_list: Option<AccessControlList>,
    /// Type-specific properties.
    pub properties: StructureProperties,
}

// ---------------------------------------------------------------------------
// 9.1 WorldSnapshot
// ---------------------------------------------------------------------------

/// End-of-tick summary of the world state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct WorldSnapshot {
    /// The tick this snapshot represents.
    pub tick: u64,
    /// Current civilizational era.
    pub era: Era,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
    /// Population metrics.
    pub population: PopulationStats,
    /// Economic metrics.
    pub economy: EconomyStats,
    /// All discoveries made to date.
    pub discoveries: Vec<String>,
    /// Narrative summary of the tick.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// 9.2 PopulationStats
// ---------------------------------------------------------------------------

/// Population metrics for a world snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PopulationStats {
    /// Number of living agents.
    pub total_alive: u32,
    /// Number of deceased agents.
    pub total_dead: u32,
    /// Agents born this tick.
    pub births_this_tick: u32,
    /// Agents who died this tick.
    pub deaths_this_tick: u32,
    /// Mean age of living agents.
    #[ts(as = "String")]
    pub average_age: Decimal,
    /// Identifier of the longest-lived agent.
    pub oldest_agent: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// 9.3 EconomyStats
// ---------------------------------------------------------------------------

/// Economic metrics for a world snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EconomyStats {
    /// Total resources across the entire simulation.
    pub total_resources: BTreeMap<Resource, u32>,
    /// Resources currently held by agents.
    pub resources_in_circulation: BTreeMap<Resource, u32>,
    /// Resources at location nodes.
    pub resources_at_nodes: BTreeMap<Resource, u32>,
    /// Number of trades completed this tick.
    pub trades_this_tick: u32,
    /// Wealth inequality coefficient (0.0 = perfect equality, 1.0 = maximum inequality).
    #[ts(as = "String")]
    pub gini_coefficient: Decimal,
}

// ---------------------------------------------------------------------------
// 5.4 Event Detail types
// ---------------------------------------------------------------------------

/// Details for a successful action event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActionSucceededDetails {
    /// The type of action that succeeded.
    pub action_type: crate::enums::ActionType,
    /// Action-specific parameters.
    pub parameters: serde_json::Value,
    /// The outcome of the action.
    pub outcome: serde_json::Value,
}

/// Details for a rejected action event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActionRejectedDetails {
    /// The type of action that was rejected.
    pub action_type: crate::enums::ActionType,
    /// Action-specific parameters.
    pub parameters: serde_json::Value,
    /// Why the action was rejected.
    pub reason: crate::enums::RejectionReason,
    /// Additional context about the rejection.
    pub reason_details: serde_json::Value,
}

/// Details for a resource gathered event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ResourceGatheredDetails {
    /// The type of resource gathered.
    pub resource: Resource,
    /// Quantity gathered.
    pub quantity: u32,
    /// Location where gathering occurred.
    pub location_id: LocationId,
    /// Skill experience points gained.
    pub skill_xp_gained: u32,
}

/// Details for a completed trade event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TradeCompletedDetails {
    /// Unique trade identifier.
    pub trade_id: TradeId,
    /// First trading party.
    pub agent_a: AgentId,
    /// Second trading party.
    pub agent_b: AgentId,
    /// Resources given by agent A.
    pub gave: BTreeMap<Resource, u32>,
    /// Resources received by agent A.
    pub received: BTreeMap<Resource, u32>,
}

/// A pending trade between two agents stored in `Dragonfly`.
///
/// Created when an agent submits a [`TradeOffer`] action. The trade remains
/// pending until the target agent accepts, rejects, or the offer expires
/// after `expires_at_tick`.
///
/// [`TradeOffer`]: crate::ActionType::TradeOffer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct PendingTrade {
    /// Unique trade identifier.
    pub trade_id: TradeId,
    /// Agent who proposed the trade.
    pub offerer_id: AgentId,
    /// Agent who must accept or reject.
    pub target_id: AgentId,
    /// Resources the offerer is giving.
    pub offered_resources: BTreeMap<Resource, u32>,
    /// Resources the offerer is requesting in return.
    pub requested_resources: BTreeMap<Resource, u32>,
    /// Tick when the trade was created.
    pub created_at_tick: u64,
    /// Tick when the trade expires if not acted upon.
    pub expires_at_tick: u64,
    /// Location where both agents were when the trade was proposed.
    pub location_id: LocationId,
}

/// The reason a trade failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum TradeFailReason {
    /// The target agent explicitly rejected the offer.
    Rejected,
    /// The trade expired before the target responded.
    Expired,
    /// One or both agents lacked the required resources at execution time.
    InsufficientResources,
    /// The agents are no longer at the same location.
    NotCoLocated,
    /// The trade was not found (already resolved or invalid ID).
    NotFound,
}

/// Details for a failed trade event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TradeFailedDetails {
    /// Unique trade identifier.
    pub trade_id: TradeId,
    /// Why the trade failed.
    pub reason: TradeFailReason,
    /// Agent who proposed the trade.
    pub offerer_id: AgentId,
    /// Agent who was the target.
    pub target_id: AgentId,
}

/// Details for a knowledge discovered event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct KnowledgeDiscoveredDetails {
    /// The concept that was discovered.
    pub knowledge: String,
    /// How it was discovered (experimentation, observation, accidental).
    pub method: String,
    /// Prerequisite concepts.
    pub prerequisites: Vec<String>,
}

/// Details for a knowledge taught event between two agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct KnowledgeTaughtDetails {
    /// The concept that was taught.
    pub knowledge: String,
    /// The agent who taught the concept.
    pub teacher_id: AgentId,
    /// The agent who received the concept.
    pub student_id: AgentId,
    /// Whether the teaching was successful.
    pub success: bool,
}

/// Details for an agent death event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AgentDiedDetails {
    /// Cause of death (starvation, old\_age, injury).
    pub cause: String,
    /// Age at time of death in ticks.
    pub final_age: u32,
    /// Inventory dropped at the death location.
    pub inventory_dropped: BTreeMap<Resource, u32>,
    /// Structures that lost their owner.
    pub structures_orphaned: Vec<StructureId>,
}

/// Details for a visible structure in perception surroundings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct VisibleStructure {
    /// The type of structure (e.g. "shelter (basic hut)").
    pub structure_type: String,
    /// Who owns it.
    pub owner: String,
    /// Current durability as a percentage string.
    pub durability: String,
    /// Names of occupants.
    pub occupants: Vec<String>,
}

/// A broadcast message visible at a location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct VisibleMessage {
    /// Who sent the message.
    pub from: String,
    /// The tick the message was sent.
    pub tick: u64,
    /// Message content.
    pub content: String,
}

/// A message stored on a location's message board in Dragonfly.
///
/// Covers both direct (`communicate`) and broadcast messages. Direct
/// messages have a non-`None` `recipient_id`. Broadcast messages have
/// `is_broadcast == true` and `recipient_id == None`.
///
/// Messages expire after a configurable number of ticks (default 10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Message {
    /// The agent who sent the message.
    pub sender_id: AgentId,
    /// Display name of the sender (for perception rendering).
    pub sender_name: String,
    /// The intended recipient, if this is a direct message.
    pub recipient_id: Option<AgentId>,
    /// Message content (max 500 characters).
    pub content: String,
    /// The tick when the message was sent.
    pub tick: u64,
    /// Whether this is a broadcast (visible to all at the location).
    pub is_broadcast: bool,
    /// The location where the message was posted.
    pub location_id: LocationId,
}

/// Details of why an action was rejected, returned to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RejectionDetails {
    /// The rejection reason code.
    pub reason: crate::enums::RejectionReason,
    /// Human-readable explanation.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Social Graph types (agent-system.md section 3.4)
// ---------------------------------------------------------------------------

/// The cause of a relationship score change between two agents.
///
/// Used to track why a relationship evolved, enabling richer social
/// dynamics and event logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum InteractionCause {
    /// A trade was successfully completed between the agents.
    Trade,
    /// A trade was rejected or failed between the agents.
    TradeFailed,
    /// Knowledge was taught from one agent to another.
    Teaching,
    /// A positive communication exchange occurred.
    Communication,
    /// A conflict occurred between the agents.
    Conflict,
}

/// Details for a relationship change event.
///
/// Emitted whenever a relationship score is updated between two agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RelationshipChangedDetails {
    /// First agent in the relationship.
    pub agent_a: AgentId,
    /// Second agent in the relationship.
    pub agent_b: AgentId,
    /// Score before the change.
    #[ts(as = "String")]
    pub old_score: Decimal,
    /// Score after the change.
    #[ts(as = "String")]
    pub new_score: Decimal,
    /// What caused the relationship to change.
    pub cause: InteractionCause,
}

/// Details for a group formation event.
///
/// Emitted when agents successfully form a new social group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct GroupFormedDetails {
    /// Unique identifier for the new group.
    pub group_id: GroupId,
    /// Display name of the group.
    pub group_name: String,
    /// The agent who founded the group.
    pub founder: AgentId,
    /// All members of the group (including the founder).
    pub members: BTreeSet<AgentId>,
    /// The tick when the group was formed.
    pub tick: u64,
}

/// A social group formed by agents.
///
/// Groups are created via the `FormGroup` action and represent voluntary
/// associations of agents with a shared identity. All members must be
/// co-located at formation and have a relationship score above 0.3 with
/// the founder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Group {
    /// Unique group identifier.
    pub id: GroupId,
    /// Display name of the group.
    pub name: String,
    /// The agent who founded the group.
    pub founder: AgentId,
    /// All current members (including the founder).
    pub members: BTreeSet<AgentId>,
    /// The tick when the group was formed.
    pub formed_at_tick: u64,
}

// ---------------------------------------------------------------------------
// Structure Blueprint (world-engine.md section 5.2)
// ---------------------------------------------------------------------------

/// A blueprint defining the cost, properties, and requirements for building
/// a specific [`StructureType`].
///
/// Blueprints are static data looked up by structure type. They define what
/// materials are required, what knowledge the builder needs, and the
/// properties of the resulting structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureBlueprint {
    /// The structure type this blueprint produces.
    pub structure_type: StructureType,
    /// Functional category of the structure.
    pub category: crate::enums::StructureCategory,
    /// Material costs required to build this structure.
    pub material_costs: BTreeMap<Resource, u32>,
    /// Knowledge concept the builder must possess.
    pub required_knowledge: String,
    /// Maximum durability of the completed structure.
    pub max_durability: u32,
    /// Durability lost per tick due to natural decay.
    #[ts(as = "String")]
    pub decay_per_tick: Decimal,
    /// Maximum number of occupants.
    pub capacity: u32,
    /// Properties of the completed structure.
    pub properties: StructureProperties,
}

// ---------------------------------------------------------------------------
// Location Effects (world-engine.md section 5)
// ---------------------------------------------------------------------------

/// Aggregate effects of all structures at a location.
///
/// Computed from the set of standing structures to determine bonuses
/// for agents resting, weather protection, production output, and
/// additional storage capacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct LocationEffects {
    /// Whether any structure at the location provides weather protection.
    pub weather_protection: bool,
    /// The best rest bonus multiplier available (as percentage, 100 = no bonus).
    pub best_rest_bonus_pct: u32,
    /// Total additional storage slots provided by structures.
    pub total_storage_slots: u32,
    /// Whether at least one shelter structure exists.
    pub has_shelter: bool,
    /// Whether a campfire (or fire source) exists.
    pub has_fire: bool,
    /// Resources produced per tick by structures at this location.
    pub production: BTreeMap<Resource, u32>,
}

// ---------------------------------------------------------------------------
// Structure Event Details
// ---------------------------------------------------------------------------

/// Details for a structure built event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureBuiltDetails {
    /// The unique ID of the newly built structure.
    pub structure_id: StructureId,
    /// The type of structure that was built.
    pub structure_type: StructureType,
    /// The location where the structure was built.
    pub location_id: LocationId,
    /// The agent who built it.
    pub builder: AgentId,
    /// Materials consumed during construction.
    pub materials_used: BTreeMap<Resource, u32>,
}

/// Details for a structure repaired event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureRepairedDetails {
    /// The structure that was repaired.
    pub structure_id: StructureId,
    /// The agent who performed the repair.
    pub repairer: AgentId,
    /// Durability before the repair.
    pub durability_before: u32,
    /// Durability after the repair.
    pub durability_after: u32,
    /// Materials consumed during repair.
    pub materials_used: BTreeMap<Resource, u32>,
}

/// Details for a structure destroyed event.
///
/// Covers both collapse from decay and demolition by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureDestroyedDetails {
    /// The structure that was destroyed.
    pub structure_id: StructureId,
    /// The type of structure that was destroyed.
    pub structure_type: StructureType,
    /// The location where the structure stood.
    pub location_id: LocationId,
    /// How the structure was destroyed.
    pub cause: String,
    /// Materials salvaged (30% of original, returned to agent or location).
    pub materials_salvaged: BTreeMap<Resource, u32>,
}

// ---------------------------------------------------------------------------
// Route Event Details (Phase 4.3)
// ---------------------------------------------------------------------------

/// Details for a route improved event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RouteImprovedDetails {
    /// The route that was improved.
    pub route_id: RouteId,
    /// The agent who performed the improvement.
    pub builder: AgentId,
    /// The previous path type before the upgrade.
    pub old_path_type: crate::enums::PathType,
    /// The new path type after the upgrade.
    pub new_path_type: crate::enums::PathType,
    /// Materials consumed during the improvement.
    pub materials_used: BTreeMap<Resource, u32>,
    /// Whether this was a repair (durability restore) rather than an upgrade.
    pub is_repair: bool,
    /// Durability restored (only relevant for repairs).
    pub durability_restored: u32,
}

/// Details for a route degraded event (durability reached zero).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RouteDegradedDetails {
    /// The route that degraded.
    pub route_id: RouteId,
    /// The previous path type before degradation.
    pub old_path_type: crate::enums::PathType,
    /// The new path type after degradation.
    pub new_path_type: crate::enums::PathType,
    /// Weather at the time of degradation.
    pub weather: Weather,
}

// ---------------------------------------------------------------------------
// Governance Types (Phase 4.4)
// ---------------------------------------------------------------------------

/// A governance rule created by a group via the `Legislate` action.
///
/// Rules are associated with a [`Group`] and created at a `MeetingHall`
/// structure. They represent social contracts that group members can
/// enforce against other agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Rule {
    /// Unique rule identifier.
    pub id: RuleId,
    /// The group that created this rule.
    pub group_id: GroupId,
    /// The agent who proposed and created the rule.
    pub creator: AgentId,
    /// Display name of the rule.
    pub name: String,
    /// Detailed description of what the rule mandates or prohibits.
    pub description: String,
    /// The tick when the rule was created.
    pub created_at_tick: u64,
    /// Whether the rule is currently active.
    pub active: bool,
}

/// Details for a structure claimed event.
///
/// Emitted when an agent takes ownership of an unowned or orphaned structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StructureClaimedDetails {
    /// The structure that was claimed.
    pub structure_id: StructureId,
    /// The type of structure that was claimed.
    pub structure_type: StructureType,
    /// The agent who claimed the structure.
    pub new_owner: AgentId,
    /// The previous owner, if any (dead agent whose ownership lapsed).
    pub previous_owner: Option<AgentId>,
    /// The location where the structure exists.
    pub location_id: LocationId,
}

/// Details for a rule created event.
///
/// Emitted when an agent successfully creates a governance rule via
/// the `Legislate` action at a `MeetingHall`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RuleCreatedDetails {
    /// The unique ID of the new rule.
    pub rule_id: RuleId,
    /// The group this rule belongs to.
    pub group_id: GroupId,
    /// The agent who created the rule.
    pub creator: AgentId,
    /// Display name of the rule.
    pub rule_name: String,
    /// Description of the rule.
    pub rule_description: String,
}

/// Details for an enforcement applied event.
///
/// Emitted when an agent enforces a governance rule against a target agent.
/// The consequence is social/reputational rather than mechanically binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EnforcementAppliedDetails {
    /// The rule being enforced.
    pub rule_id: RuleId,
    /// The agent performing the enforcement.
    pub enforcer: AgentId,
    /// The agent the rule is being enforced against.
    pub target: AgentId,
    /// The group whose authority backs this enforcement.
    pub group_id: GroupId,
    /// A description of the consequence applied.
    pub consequence: String,
}
