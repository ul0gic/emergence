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

export type Sex = "Male" | "Female";

export interface Agent {
  id: AgentId;
  name: string;
  sex: Sex;
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
  thirst: number;
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
  sex?: Sex;
  born_at_tick: number;
  died_at_tick: number | null;
  cause_of_death?: string | null;
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

export interface RoutesResponse {
  count: number;
  routes: Route[];
}

export interface EventsResponse {
  count: number;
  events: Event[];
}

// ---------------------------------------------------------------------------
// Operator types (for simulation management)
// ---------------------------------------------------------------------------

export type InjectedEventType =
  | "natural_disaster"
  | "resource_boom"
  | "plague"
  | "migration"
  | "technology_gift"
  | "resource_depletion";

export interface OperatorStatus {
  tick: number;
  paused: boolean;
  stop_requested: boolean;
  tick_interval_ms: number;
  elapsed_seconds: number;
  max_ticks: number;
  max_real_time_seconds: number;
  agents_alive: number;
  agents_total: number;
  end_reason: string | null;
  started_at: string;
}

export interface InjectEventRequest {
  event_type: InjectedEventType;
  target_region?: string;
  parameters?: Record<string, unknown>;
}

export interface OperatorMutationResponse {
  success: boolean;
  message: string;
}

// ---------------------------------------------------------------------------
// Social construct types (Phase 6.4 — emergent social constructs)
// ---------------------------------------------------------------------------

export type SocialConstructCategory =
  | "religion"
  | "governance"
  | "family"
  | "economy"
  | "crime_justice";

export interface SocialConstruct {
  id: string;
  name: string;
  category: SocialConstructCategory;
  adherent_count: number;
  founded_at_tick: number;
  properties: Record<string, string>;
}

export interface BeliefSystem {
  id: string;
  name: string;
  themes: string[];
  adherent_count: number;
  founded_at_tick: number;
}

export interface BeliefEvent {
  tick: number;
  event_type: "founded" | "schism" | "merged" | "converted";
  belief_system_id: string;
  belief_system_name: string;
  description: string;
  agent_id: string | null;
}

export type GovernanceType =
  | "Anarchy"
  | "Chieftainship"
  | "Council"
  | "Monarchy"
  | "Democracy"
  | "Oligarchy"
  | "Theocracy";

export interface GovernanceLeader {
  agent_id: string;
  agent_name: string;
  role: string;
  since_tick: number;
}

export interface GovernanceEvent {
  tick: number;
  event_type: "election" | "coup" | "declaration" | "succession" | "reform";
  description: string;
  agent_id: string | null;
}

export interface GovernanceInfo {
  governance_type: GovernanceType;
  leaders: GovernanceLeader[];
  rules: string[];
  stability_score: number;
  recent_events: GovernanceEvent[];
}

export interface FamilyUnit {
  id: string;
  name: string;
  members: string[];
  head: string;
  formed_at_tick: number;
}

export interface LineageNode {
  agent_id: string;
  agent_name: string;
  parent_a: string | null;
  parent_b: string | null;
  generation: number;
  alive: boolean;
  children: string[];
}

export interface FamilyStats {
  unit_count: number;
  avg_size: number;
  marriage_count: number;
  divorce_count: number;
  orphan_count: number;
  longest_lineage: number;
  families: FamilyUnit[];
  lineage: LineageNode[];
}

export type EconomicModelType = "Subsistence" | "Gift" | "Barter" | "Currency" | "Market" | "Mixed";

export interface MarketLocation {
  location_id: string;
  location_name: string;
  trade_volume: number;
  primary_resource: Resource;
}

export interface EconomicClassification {
  model_type: EconomicModelType;
  currency_resource: Resource | null;
  currency_adoption_pct: number;
  trade_volume: number;
  trade_volume_history: { tick: number; volume: number }[];
  market_locations: MarketLocation[];
}

export type JusticeType = "None" | "Vigilante" | "Elder" | "Council" | "Codified" | "Institutional";

export interface CrimeEntry {
  crime_type: string;
  count: number;
}

export interface SerialOffender {
  agent_id: string;
  agent_name: string;
  offense_count: number;
  last_offense_tick: number;
}

export interface CrimeHotspot {
  location_id: string;
  location_name: string;
  crime_count: number;
}

export interface CrimeStats {
  crime_rate: number;
  crime_rate_history: { tick: number; rate: number }[];
  detection_rate: number;
  punishment_rate: number;
  justice_type: JusticeType;
  common_crimes: CrimeEntry[];
  serial_offenders: SerialOffender[];
  hotspots: CrimeHotspot[];
}

// ---------------------------------------------------------------------------
// Social API response wrappers (match backend JSON shapes)
// ---------------------------------------------------------------------------

export interface BeliefsResponse {
  belief_systems: BeliefSystem[];
  belief_events: BeliefEvent[];
}

export interface SocialEconomyResponse {
  model_type: EconomicModelType;
  currency_resource: Resource | null;
  currency_adoption_pct: number;
  trade_volume: number;
  trade_volume_history: { tick: number; volume: number }[];
  market_locations: MarketLocation[];
}

// ---------------------------------------------------------------------------
// Civilization timeline event (cross-construct emergence tracking)
// ---------------------------------------------------------------------------

export type CivilizationMilestoneCategory =
  | "belief"
  | "governance"
  | "family"
  | "economy"
  | "crime";

export interface CivilizationMilestone {
  tick: number;
  category: CivilizationMilestoneCategory;
  label: string;
  description: string;
}

// ---------------------------------------------------------------------------
// Decision record types (Phase 9.3 — LLM Decision Viewer)
// ---------------------------------------------------------------------------

export type DecisionSource = "llm" | "rule_engine" | "night_cycle" | "timeout";

export interface DecisionRecord {
  agent_id: AgentId;
  tick: number;
  decision_source: DecisionSource;
  action_type: string;
  action_params: Record<string, unknown> | null;
  llm_backend: string | null;
  model: string | null;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  cost_usd: number | null;
  latency_ms: number | null;
  raw_llm_response: string | null;
  prompt_sent: string | null;
  rule_matched: string | null;
  created_at: string;
}

export interface DecisionsResponse {
  count: number;
  decisions: DecisionRecord[];
}
