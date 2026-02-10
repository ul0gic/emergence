//! Enumeration types for the Emergence simulation.
//!
//! All enumerations defined in `data-schemas.md` sections 3.1 through 3.10,
//! plus the ledger entry type from section 6.2.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------------------------------------------------------------
// 3.1 Resource Types
// ---------------------------------------------------------------------------

/// A resource that exists in the simulation world.
///
/// Resources are organized into tiers reflecting technological progression:
/// - Tier 0: Survival basics available from tick 0
/// - Tier 1: Developed resources requiring discovery or effort
/// - Tier 2: Advanced resources requiring multi-step processes
/// - Tier 3: Complex resources requiring civilization-level coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum Resource {
    // --- Tier 0: Survival ---
    /// Fresh water for hydration.
    Water,
    /// Wild berries gathered from bushes.
    FoodBerry,
    /// Fish caught from rivers or coastline.
    FoodFish,
    /// Edible roots dug from soil.
    FoodRoot,

    // --- Tier 1: Survival (developed) ---
    /// Meat from hunted animals.
    FoodMeat,
    /// Crops grown via agriculture.
    FoodFarmed,
    /// Food prepared with fire for higher nutritional value.
    FoodCooked,

    // --- Tier 0: Material ---
    /// Lumber harvested from forests.
    Wood,
    /// Raw stone from rocky areas.
    Stone,

    // --- Tier 1: Material ---
    /// Plant fiber for rope, baskets, and textiles.
    Fiber,
    /// Malleable clay from riverbanks.
    Clay,
    /// Animal hides for clothing and shelter.
    Hide,

    // --- Tier 2: Material ---
    /// Raw ore extracted from mines.
    Ore,
    /// Refined metal smelted from ore.
    Metal,

    // --- Tier 2: Consumable ---
    /// Herbal medicine for health restoration.
    Medicine,

    // --- Tier 1: Equipment ---
    /// Basic tools crafted from wood and stone.
    Tool,

    // --- Tier 2: Equipment ---
    /// Advanced tools crafted with metal.
    ToolAdvanced,

    // --- Tier 3: Abstract ---
    /// Collectively agreed-upon medium of exchange.
    CurrencyToken,
    /// Persistent knowledge stored on a physical medium.
    WrittenRecord,
}

// ---------------------------------------------------------------------------
// 3.2 Structure Types
// ---------------------------------------------------------------------------

/// A type of structure that can be built at a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum StructureType {
    // --- Tier 0 ---
    /// A fire for warmth, cooking, and light.
    Campfire,
    /// A minimal shelter providing basic rest bonus.
    LeanTo,
    /// A full shelter with weather protection and storage.
    BasicHut,

    // --- Tier 1 ---
    /// Underground storage for extra inventory at a location.
    StoragePit,
    /// A reliable water source independent of rivers.
    Well,
    /// Agricultural plot for growing crops.
    FarmPlot,
    /// Workbench for crafting tools and processed materials.
    Workshop,
    /// Gathering place for group decisions and governance.
    MeetingHall,

    // --- Tier 2 ---
    /// High-temperature facility for smelting ore into metal.
    Forge,
    /// Knowledge repository for reading and writing records.
    Library,
    /// Formal trading venue with price memory.
    Market,
    /// Defensive fortification restricting location access.
    Wall,
    /// Infrastructure connecting locations across obstacles.
    Bridge,
}

/// The functional category of a structure.
///
/// Determines the primary purpose of the structure within the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum StructureCategory {
    /// Provides warmth, light, or cooking utility.
    Utility,
    /// Provides shelter and rest bonuses.
    Shelter,
    /// Provides extra inventory storage at a location.
    Storage,
    /// Produces resources over time.
    Production,
    /// Enables social functions (governance, groups).
    Social,
    /// Stores and retrieves knowledge.
    Knowledge,
    /// Supports formal economic exchanges.
    Economic,
    /// Provides defensive capability.
    Defense,
    /// Provides transportation infrastructure.
    Infrastructure,
}

// ---------------------------------------------------------------------------
// 3.3 Action Types
// ---------------------------------------------------------------------------

/// An action that an agent can submit to the World Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum ActionType {
    // --- Survival ---
    /// Collect resources from the current location.
    Gather,
    /// Consume food to reduce hunger and restore energy.
    Eat,
    /// Consume water for hydration.
    Drink,
    /// Recover energy (bonus if sheltered).
    Rest,

    // --- Movement ---
    /// Travel to an adjacent location via a known route.
    Move,

    // --- Construction ---
    /// Create a new structure at the current location.
    Build,
    /// Restore durability to an existing structure.
    Repair,
    /// Destroy a structure and salvage materials.
    Demolish,
    /// Upgrade the path type of a route.
    ImproveRoute,

    // --- Social ---
    /// Send a direct message to a co-located agent.
    Communicate,
    /// Post a message visible to all agents at the location.
    Broadcast,
    /// Propose a resource exchange to another agent.
    TradeOffer,
    /// Accept a pending trade offer.
    TradeAccept,
    /// Reject a pending trade offer.
    TradeReject,
    /// Create a named social group.
    FormGroup,
    /// Transfer knowledge to another agent.
    Teach,

    // --- Advanced ---
    /// Plant crops on a farm plot.
    FarmPlant,
    /// Harvest mature crops from a farm plot.
    FarmHarvest,
    /// Create tools or processed goods at a workshop.
    Craft,
    /// Extract ore from rocky terrain.
    Mine,
    /// Convert ore to metal at a forge.
    Smelt,
    /// Persist knowledge to a library.
    Write,
    /// Acquire knowledge from a library.
    Read,
    /// Take ownership of an unowned structure or location.
    Claim,
    /// Create a rule or law via group consensus.
    Legislate,
    /// Apply consequences for rule violations.
    Enforce,
    /// Spawn a child agent with a consenting partner.
    Reproduce,

    // --- Conflict ---
    /// Take resources from a co-located agent by force or stealth.
    Steal,
    /// Engage in physical confrontation with another agent.
    Attack,
    /// Intimidate a co-located agent without dealing damage.
    Intimidate,

    // --- Diplomacy ---
    /// Propose a group decision, alliance, or treaty.
    Propose,
    /// Cast a vote on a pending group proposal.
    Vote,
    /// Enter a formal partnership with another agent.
    Marry,
    /// Dissolve a formal partnership with another agent.
    Divorce,
    /// Engage in secret coordination with a subset of agents.
    Conspire,

    // --- Spiritual ---
    /// Perform a spiritual or ritualistic action.
    Pray,

    // --- Freeform ---
    /// A novel action proposed by an agent beyond the base catalog.
    ///
    /// Freeform actions are evaluated by the feasibility engine before
    /// execution. If the engine can map the action to a known category,
    /// it resolves it; otherwise it queues it for LLM adjudication.
    Freeform,

    // --- System ---
    /// Agent did not act this tick (timeout or explicit forfeit).
    NoAction,
}

// ---------------------------------------------------------------------------
// 3.4 Event Types
// ---------------------------------------------------------------------------

/// A type of event recorded in the event store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum EventType {
    // --- System ---
    /// Beginning of a tick.
    TickStart,
    /// End of a tick.
    TickEnd,

    // --- Lifecycle ---
    /// A new agent was created.
    AgentBorn,
    /// An agent died.
    AgentDied,

    // --- Action ---
    /// An agent submitted an action.
    ActionSubmitted,
    /// An action completed successfully.
    ActionSucceeded,
    /// An action failed validation.
    ActionRejected,

    // --- Economy ---
    /// An agent collected resources from a location.
    ResourceGathered,
    /// An agent consumed resources.
    ResourceConsumed,
    /// Two agents completed a resource exchange.
    TradeCompleted,
    /// A trade was rejected or deemed invalid.
    TradeFailed,

    // --- World ---
    /// A new structure was created.
    StructureBuilt,
    /// A structure collapsed or was demolished.
    StructureDestroyed,
    /// A structure's durability was restored.
    StructureRepaired,
    /// A route's path type was upgraded.
    RouteImproved,
    /// A route's durability reached zero and it degraded to a lower path type.
    RouteDegraded,
    /// An agent discovered a previously unknown location.
    LocationDiscovered,

    // --- Knowledge ---
    /// An agent learned something new.
    KnowledgeDiscovered,
    /// Knowledge was transferred between agents.
    KnowledgeTaught,

    // --- Social ---
    /// An agent sent a message.
    MessageSent,
    /// A new social group was formed.
    GroupFormed,
    /// A relationship score was updated.
    RelationshipChanged,

    // --- Governance ---
    /// An agent claimed ownership of a structure.
    StructureClaimed,
    /// A governance rule was created by a group.
    RuleCreated,
    /// A governance rule was enforced against an agent.
    EnforcementApplied,

    // --- Environment ---
    /// The weather changed.
    WeatherChanged,
    /// The season transitioned.
    SeasonChanged,

    // --- Conflict ---
    /// A theft was successfully committed (resources transferred).
    TheftOccurred,
    /// A theft attempt failed (caught or insufficient resources).
    TheftFailed,
    /// A combat encounter was initiated.
    CombatInitiated,
    /// A combat encounter was resolved (winner determined, damage applied).
    CombatResolved,

    // --- System (alert) ---
    /// Conservation law violated -- critical ledger alert.
    LedgerAnomaly,
}

// ---------------------------------------------------------------------------
// 3.5 Rejection Reasons
// ---------------------------------------------------------------------------

/// The reason an agent's action was rejected by the World Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum RejectionReason {
    /// Action type not recognized.
    InvalidAction,
    /// Agent lacks energy for this action.
    InsufficientEnergy,
    /// Agent is not at the required location.
    WrongLocation,
    /// Agent lacks the required materials.
    InsufficientResources,
    /// Target resource, agent, or structure is not available.
    UnavailableTarget,
    /// Agent lacks the knowledge to perform this action.
    UnknownAction,
    /// Another agent won the contested resource.
    ConflictLost,
    /// Action would exceed the agent's carry capacity.
    CapacityExceeded,
    /// Target agent or structure does not exist.
    InvalidTarget,
    /// Access control list prevents this action.
    PermissionDenied,
    /// Agent missed the decision deadline.
    Timeout,
    /// A freeform action was deemed physically impossible.
    Infeasible,
    /// A freeform action is too ambiguous for rule-based evaluation.
    NeedsEvaluation,
}

// ---------------------------------------------------------------------------
// 3.6 Seasons
// ---------------------------------------------------------------------------

/// A season in the simulation's annual cycle (90 ticks per season by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum Season {
    /// Resource regeneration +25%.
    Spring,
    /// Normal resource rates.
    Summer,
    /// Harvest +50%, regeneration -25%.
    Autumn,
    /// Regeneration -75%, hunger +50%.
    Winter,
}

// ---------------------------------------------------------------------------
// 3.7 Weather
// ---------------------------------------------------------------------------

/// Current weather conditions affecting travel, structures, and farming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum Weather {
    /// No weather effects.
    Clear,
    /// Travel +1 tick cost, farm growth +25%.
    Rain,
    /// Travel blocked, structure decay +100%, farm damage risk.
    Storm,
    /// Farm growth stopped.
    Drought,
    /// Travel +2 tick cost, structure decay +50%, farm growth stopped.
    Snow,
}

// ---------------------------------------------------------------------------
// 3.8 Path Types
// ---------------------------------------------------------------------------

/// The quality of a route between two locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum PathType {
    /// Wilderness with no path (8 tick base cost).
    None,
    /// Basic cleared path (5 tick base cost).
    DirtTrail,
    /// Established foot traffic (3 tick base cost).
    WornPath,
    /// Constructed road (2 tick base cost).
    Road,
    /// Major infrastructure (1 tick base cost).
    Highway,
}

// ---------------------------------------------------------------------------
// 3.9 Time of Day
// ---------------------------------------------------------------------------

/// The time of day within a single tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum TimeOfDay {
    /// Transition from night to day; rest bonus ending.
    Dawn,
    /// Normal perception and energy.
    Morning,
    /// Normal perception and energy.
    Afternoon,
    /// Transition from day to night.
    Dusk,
    /// Reduced perception radius, action cost +25%, rest bonus +50%.
    Night,
}

// ---------------------------------------------------------------------------
// 3.10 Eras
// ---------------------------------------------------------------------------

/// The current civilizational era, determined by emergent agent behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum Era {
    /// Starting era -- no organized society.
    Primitive,
    /// Group formation has emerged.
    Tribal,
    /// Farming has been discovered.
    Agricultural,
    /// Permanent structures are established.
    Settlement,
    /// Metalworking has been discovered.
    Bronze,
    /// Advanced metalworking.
    Iron,
    /// Written language and governance.
    Classical,
    /// Complex institutions.
    Medieval,
    /// Manufacturing (if reached).
    Industrial,
    /// Full technology (if reached).
    Modern,
}

// ---------------------------------------------------------------------------
// 6.2 Ledger Entry Types
// ---------------------------------------------------------------------------

/// The category of a resource transfer in the central ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum LedgerEntryType {
    /// Resource respawned at a location (world -> location).
    Regeneration,
    /// Agent collected resource from location (location -> agent).
    Gather,
    /// Agent consumed resource (agent -> void).
    Consume,
    /// Resource exchanged between agents (agent -> agent).
    Transfer,
    /// Construction material used (agent -> structure).
    Build,
    /// Demolition material recovered (structure -> agent).
    Salvage,
    /// Structure degradation loss (structure -> void).
    Decay,
    /// Inventory dropped on death (agent -> location).
    Drop,
    /// Scavenging dropped items (location -> agent).
    Pickup,
    /// Resources stolen from one agent to another (agent -> agent).
    Theft,
    /// Resources looted from a defeated agent (agent -> agent).
    CombatLoot,
}

// ---------------------------------------------------------------------------
// Entity type for ledger from/to fields
// ---------------------------------------------------------------------------

/// The type of entity participating in a ledger transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum EntityType {
    /// An agent in the simulation.
    Agent,
    /// A location node in the world graph.
    Location,
    /// A built structure.
    Structure,
    /// The world itself (source of regeneration).
    World,
    /// The void (destination for consumption and decay).
    Void,
}

// ---------------------------------------------------------------------------
// Memory tier
// ---------------------------------------------------------------------------

/// The retention tier of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum MemoryTier {
    /// Full detail, retained for last 5 ticks.
    Immediate,
    /// Summarized, retained for last 50 ticks.
    ShortTerm,
    /// Major milestones, retained for lifetime.
    LongTerm,
}
