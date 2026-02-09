/**
 * Generated types from Rust via ts-rs, adapted for JSON wire format.
 *
 * The Rust ts-rs bindings use `bigint` for u64 fields, but the JSON wire
 * format from serde_json uses regular numbers. These types reflect what
 * actually arrives over the wire.
 *
 * IMPORTANT: Do not manually add types here that exist in the Rust bindings.
 * If the Rust types change, regenerate from the bindings directory.
 */

// ---------------------------------------------------------------------------
// ID types (all string UUIDs)
// ---------------------------------------------------------------------------

export type AgentId = string;
export type LocationId = string;
export type RouteId = string;
export type StructureId = string;
export type EventId = string;
export type TradeId = string;
export type GroupId = string;
export type LedgerEntryId = string;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

export type Resource =
  | "Water"
  | "FoodBerry"
  | "FoodFish"
  | "FoodRoot"
  | "FoodMeat"
  | "FoodFarmed"
  | "FoodCooked"
  | "Wood"
  | "Stone"
  | "Fiber"
  | "Clay"
  | "Hide"
  | "Ore"
  | "Metal"
  | "Medicine"
  | "Tool"
  | "ToolAdvanced"
  | "CurrencyToken"
  | "WrittenRecord";

export type Era =
  | "Primitive"
  | "Tribal"
  | "Agricultural"
  | "Settlement"
  | "Bronze"
  | "Iron"
  | "Classical"
  | "Medieval"
  | "Industrial"
  | "Modern";

export type Season = "Spring" | "Summer" | "Autumn" | "Winter";

export type Weather = "Clear" | "Rain" | "Storm" | "Drought" | "Snow";

export type PathType = "None" | "DirtTrail" | "WornPath" | "Road" | "Highway";

export type StructureType =
  | "Campfire"
  | "LeanTo"
  | "BasicHut"
  | "StoragePit"
  | "Well"
  | "FarmPlot"
  | "Workshop"
  | "MeetingHall"
  | "Forge"
  | "Library"
  | "Market"
  | "Wall"
  | "Bridge";

export type EventType =
  | "TickStart"
  | "TickEnd"
  | "AgentBorn"
  | "AgentDied"
  | "ActionSubmitted"
  | "ActionSucceeded"
  | "ActionRejected"
  | "ResourceGathered"
  | "ResourceConsumed"
  | "TradeCompleted"
  | "TradeFailed"
  | "StructureBuilt"
  | "StructureDestroyed"
  | "StructureRepaired"
  | "RouteImproved"
  | "LocationDiscovered"
  | "KnowledgeDiscovered"
  | "KnowledgeTaught"
  | "MessageSent"
  | "GroupFormed"
  | "RelationshipChanged"
  | "WeatherChanged"
  | "SeasonChanged"
  | "LedgerAnomaly";

export type MemoryTier = "Immediate" | "ShortTerm" | "LongTerm";

export type ActionType =
  | "Gather"
  | "Eat"
  | "Drink"
  | "Rest"
  | "Move"
  | "Build"
  | "Repair"
  | "Demolish"
  | "ImproveRoute"
  | "Communicate"
  | "Broadcast"
  | "TradeOffer"
  | "TradeAccept"
  | "TradeReject"
  | "FormGroup"
  | "Teach"
  | "FarmPlant"
  | "FarmHarvest"
  | "Craft"
  | "Mine"
  | "Smelt"
  | "Write"
  | "Read"
  | "Claim"
  | "Legislate"
  | "Enforce"
  | "Reproduce"
  | "NoAction";

export type RejectionReason =
  | "InvalidAction"
  | "InsufficientEnergy"
  | "WrongLocation"
  | "InsufficientResources"
  | "UnavailableTarget"
  | "UnknownAction"
  | "ConflictLost"
  | "CapacityExceeded"
  | "InvalidTarget"
  | "PermissionDenied"
  | "Timeout";

export type LedgerEntryType =
  | "Regeneration"
  | "Gather"
  | "Consume"
  | "Transfer"
  | "Build"
  | "Salvage"
  | "Decay"
  | "Drop"
  | "Pickup";

export type EntityType = "agent" | "location" | "structure" | "world" | "void";

// ---------------------------------------------------------------------------
// Core entity types
// ---------------------------------------------------------------------------

export interface Personality {
  curiosity: string;
  cooperation: string;
  aggression: string;
  risk_tolerance: string;
  industriousness: string;
  sociability: string;
  honesty: string;
  loyalty: string;
}

export interface Agent {
  id: AgentId;
  name: string;
  born_at_tick: number;
  died_at_tick: number | null;
  cause_of_death: string | null;
  parent_a: AgentId | null;
  parent_b: AgentId | null;
  generation: number;
  personality: Personality;
  created_at: string;
}

export interface MemoryEntry {
  tick: number;
  memory_type: string;
  summary: string;
  entities: string[];
  emotional_weight: string;
  tier: MemoryTier;
}

export interface AgentState {
  agent_id: AgentId;
  energy: number;
  health: number;
  hunger: number;
  age: number;
  born_at_tick: number;
  location_id: LocationId;
  destination_id: LocationId | null;
  travel_progress: number;
  inventory: Partial<Record<Resource, number>>;
  carry_capacity: number;
  knowledge: string[];
  skills: Record<string, number | undefined>;
  skill_xp: Record<string, number | undefined>;
  goals: string[];
  relationships: Record<AgentId, string | undefined>;
  memory: MemoryEntry[];
}

export interface ResourceNode {
  resource: Resource;
  available: number;
  regen_per_tick: number;
  max_capacity: number;
}

export interface Location {
  id: LocationId;
  name: string;
  region: string;
  location_type: string;
  description: string;
  capacity: number;
  base_resources: Partial<Record<Resource, ResourceNode>>;
  discovered_by: AgentId[];
  created_at: string;
}

export interface AccessControlList {
  allowed_agents: AgentId[];
  allowed_groups: GroupId[];
  denied_agents: AgentId[];
  public: boolean;
  toll_cost: Partial<Record<Resource, number>> | null;
}

export interface Route {
  id: RouteId;
  from_location: LocationId;
  to_location: LocationId;
  cost_ticks: number;
  path_type: PathType;
  durability: number;
  max_durability: number;
  decay_per_tick: string;
  acl: AccessControlList | null;
  bidirectional: boolean;
  built_by: AgentId | null;
  built_at_tick: number | null;
}

export interface StructureProperties {
  rest_bonus: string;
  weather_protection: boolean;
  storage_slots: number;
  production_type: Resource | null;
  production_rate: number;
}

export interface Structure {
  id: StructureId;
  structure_type: StructureType;
  subtype: string | null;
  location_id: LocationId;
  builder: AgentId;
  owner: AgentId | null;
  built_at_tick: number;
  destroyed_at_tick: number | null;
  materials_used: Partial<Record<Resource, number>>;
  durability: number;
  max_durability: number;
  decay_per_tick: string;
  capacity: number;
  occupants: AgentId[];
  access_list: AccessControlList | null;
  properties: StructureProperties;
}

export interface Group {
  id: GroupId;
  name: string;
  founder: AgentId;
  members: AgentId[];
  formed_at_tick: number;
}

// ---------------------------------------------------------------------------
// World state types
// ---------------------------------------------------------------------------

export interface PopulationStats {
  total_alive: number;
  total_dead: number;
  births_this_tick: number;
  deaths_this_tick: number;
  average_age: string;
  oldest_agent: AgentId | null;
}

export interface EconomyStats {
  total_resources: Partial<Record<Resource, number>>;
  resources_in_circulation: Partial<Record<Resource, number>>;
  resources_at_nodes: Partial<Record<Resource, number>>;
  trades_this_tick: number;
  gini_coefficient: string;
}

export interface WorldContext {
  tick: number;
  era: Era;
  season: Season;
  weather: Weather;
  population: number;
}

export interface AgentStateSnapshot {
  energy: number;
  health: number;
  hunger: number;
  age: number;
  location_id: LocationId;
  inventory_summary: Partial<Record<Resource, number>>;
}

export interface WorldSnapshot {
  tick: number;
  era: Era;
  season: Season;
  weather: Weather;
  population: PopulationStats;
  economy: EconomyStats;
  discoveries: string[];
  summary: string;
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

export type JsonValue =
  | number
  | string
  | boolean
  | JsonValue[]
  | { [key: string]: JsonValue | undefined }
  | null;

export interface Event {
  id: EventId;
  tick: number;
  event_type: EventType;
  agent_id: AgentId | null;
  location_id: LocationId | null;
  details: JsonValue;
  agent_state_snapshot: AgentStateSnapshot | null;
  world_context: WorldContext;
  created_at: string;
}

export interface KnowledgeDiscoveredDetails {
  knowledge: string;
  method: string;
  prerequisites: string[];
}

export interface TradeCompletedDetails {
  trade_id: TradeId;
  agent_a: AgentId;
  agent_b: AgentId;
  gave: Partial<Record<Resource, number>>;
  received: Partial<Record<Resource, number>>;
}

export interface AgentDiedDetails {
  cause: string;
  final_age: number;
  inventory_dropped: Partial<Record<Resource, number>>;
  structures_orphaned: StructureId[];
}

// ---------------------------------------------------------------------------
// WebSocket tick broadcast (matches Rust TickBroadcast in state.rs)
// ---------------------------------------------------------------------------

export interface TickBroadcast {
  tick: number;
  season: Season;
  weather: Weather;
  agents_alive: number;
  deaths_this_tick: number;
  actions_resolved: number;
}

// ---------------------------------------------------------------------------
// API response wrappers
// ---------------------------------------------------------------------------

export interface AgentListItem {
  id: AgentId;
  name: string;
  born_at_tick: number;
  died_at_tick: number | null;
  generation: number;
  alive: boolean;
  vitals: {
    energy: number;
    health: number;
    hunger: number;
    age: number;
  } | null;
  location_id: LocationId | null;
}

export interface AgentsResponse {
  count: number;
  agents: AgentListItem[];
}

export interface AgentDetailResponse {
  agent: Agent;
  state: AgentState | null;
}

export interface LocationListItem {
  id: LocationId;
  name: string;
  region: string;
  location_type: string;
  capacity: number;
}

export interface LocationsResponse {
  count: number;
  locations: LocationListItem[];
}

export interface LocationDetailResponse {
  location: Location;
  agents_here: { id: AgentId; name: string }[];
}

export interface EventsResponse {
  count: number;
  events: Event[];
}
