/**
 * Zod runtime validation schemas for all data arriving over WebSocket and REST.
 *
 * These schemas validate against the generated TypeScript types. Every message
 * from the wire is parsed through Zod before it touches application state.
 */
import { z } from "zod/v4";

import type { DecisionsResponse, EventsResponse } from "./generated/index.ts";

// ---------------------------------------------------------------------------
// Enum schemas
// ---------------------------------------------------------------------------

export const ResourceSchema = z.enum([
  "Water",
  "FoodBerry",
  "FoodFish",
  "FoodRoot",
  "FoodMeat",
  "FoodFarmed",
  "FoodCooked",
  "Wood",
  "Stone",
  "Fiber",
  "Clay",
  "Hide",
  "Ore",
  "Metal",
  "Medicine",
  "Tool",
  "ToolAdvanced",
  "CurrencyToken",
  "WrittenRecord",
]);

export const EraSchema = z.enum([
  "Primitive",
  "Tribal",
  "Agricultural",
  "Settlement",
  "Bronze",
  "Iron",
  "Classical",
  "Medieval",
  "Industrial",
  "Modern",
]);

export const SeasonSchema = z.enum(["Spring", "Summer", "Autumn", "Winter"]);

export const WeatherSchema = z.enum(["Clear", "Rain", "Storm", "Drought", "Snow"]);

export const PathTypeSchema = z.enum(["None", "DirtTrail", "WornPath", "Road", "Highway"]);

export const EventTypeSchema = z.enum([
  "TickStart",
  "TickEnd",
  "AgentBorn",
  "AgentDied",
  "ActionSubmitted",
  "ActionSucceeded",
  "ActionRejected",
  "ResourceGathered",
  "ResourceConsumed",
  "TradeCompleted",
  "TradeFailed",
  "StructureBuilt",
  "StructureDestroyed",
  "StructureRepaired",
  "RouteImproved",
  "LocationDiscovered",
  "KnowledgeDiscovered",
  "KnowledgeTaught",
  "MessageSent",
  "GroupFormed",
  "RelationshipChanged",
  "WeatherChanged",
  "SeasonChanged",
  "LedgerAnomaly",
]);

export const MemoryTierSchema = z.enum(["Immediate", "ShortTerm", "LongTerm"]);

// ---------------------------------------------------------------------------
// ID schemas (UUID strings)
// ---------------------------------------------------------------------------

const UuidSchema = z.string();

// ---------------------------------------------------------------------------
// Core schemas
// ---------------------------------------------------------------------------

export const PersonalitySchema = z.object({
  curiosity: z.string(),
  cooperation: z.string(),
  aggression: z.string(),
  risk_tolerance: z.string(),
  industriousness: z.string(),
  sociability: z.string(),
  honesty: z.string(),
  loyalty: z.string(),
});

export const MemoryEntrySchema = z.object({
  tick: z.number(),
  memory_type: z.string(),
  summary: z.string(),
  entities: z.array(z.string()),
  emotional_weight: z.string(),
  tier: MemoryTierSchema,
});

export const SexSchema = z.enum(["Male", "Female"]);

export const AgentSchema = z.object({
  id: UuidSchema,
  name: z.string(),
  sex: SexSchema,
  born_at_tick: z.number(),
  died_at_tick: z.number().nullable(),
  cause_of_death: z.string().nullable(),
  parent_a: UuidSchema.nullable(),
  parent_b: UuidSchema.nullable(),
  generation: z.number(),
  personality: PersonalitySchema,
  created_at: z.string(),
});

export const AgentStateSchema = z.object({
  agent_id: UuidSchema,
  energy: z.number(),
  health: z.number(),
  hunger: z.number(),
  thirst: z.number(),
  age: z.number(),
  born_at_tick: z.number(),
  location_id: UuidSchema,
  destination_id: UuidSchema.nullable(),
  travel_progress: z.number(),
  inventory: z.record(ResourceSchema, z.number().optional()),
  carry_capacity: z.number(),
  knowledge: z.array(z.string()),
  skills: z.record(z.string(), z.number().optional()),
  skill_xp: z.record(z.string(), z.number().optional()),
  goals: z.array(z.string()),
  relationships: z.record(UuidSchema, z.string().optional()),
  memory: z.array(MemoryEntrySchema),
});

export const ResourceNodeSchema = z.object({
  resource: ResourceSchema,
  available: z.number(),
  regen_per_tick: z.number(),
  max_capacity: z.number(),
});

export const LocationSchema = z.object({
  id: UuidSchema,
  name: z.string(),
  region: z.string(),
  location_type: z.string(),
  description: z.string(),
  capacity: z.number(),
  base_resources: z.record(ResourceSchema, ResourceNodeSchema.optional()),
  discovered_by: z.array(UuidSchema),
  created_at: z.string(),
});

export const RouteSchema = z.object({
  id: UuidSchema,
  from_location: UuidSchema,
  to_location: UuidSchema,
  cost_ticks: z.number(),
  path_type: PathTypeSchema,
  durability: z.number(),
  max_durability: z.number(),
  decay_per_tick: z.string(),
  acl: z
    .object({
      allowed_agents: z.array(UuidSchema),
      allowed_groups: z.array(UuidSchema),
      denied_agents: z.array(UuidSchema),
      public: z.boolean(),
      toll_cost: z.record(ResourceSchema, z.number().optional()).nullable(),
    })
    .nullable(),
  bidirectional: z.boolean(),
  built_by: UuidSchema.nullable(),
  built_at_tick: z.number().nullable(),
});

// ---------------------------------------------------------------------------
// World state schemas
// ---------------------------------------------------------------------------

export const PopulationStatsSchema = z.object({
  total_alive: z.number(),
  total_dead: z.number(),
  births_this_tick: z.number(),
  deaths_this_tick: z.number(),
  average_age: z.string(),
  oldest_agent: UuidSchema.nullable(),
});

export const EconomyStatsSchema = z.object({
  total_resources: z.record(ResourceSchema, z.number().optional()),
  resources_in_circulation: z.record(ResourceSchema, z.number().optional()),
  resources_at_nodes: z.record(ResourceSchema, z.number().optional()),
  trades_this_tick: z.number(),
  gini_coefficient: z.string(),
});

export const WorldContextSchema = z.object({
  tick: z.number(),
  era: EraSchema,
  season: SeasonSchema,
  weather: WeatherSchema,
  population: z.number(),
});

export const AgentStateSnapshotSchema = z.object({
  energy: z.number(),
  health: z.number(),
  hunger: z.number(),
  age: z.number(),
  location_id: UuidSchema,
  inventory_summary: z.record(ResourceSchema, z.number().optional()),
});

export const WorldSnapshotSchema = z.object({
  tick: z.number(),
  era: EraSchema,
  season: SeasonSchema,
  weather: WeatherSchema,
  population: PopulationStatsSchema,
  economy: EconomyStatsSchema,
  discoveries: z.array(z.string()),
  summary: z.string(),
});

// ---------------------------------------------------------------------------
// Event schema
// ---------------------------------------------------------------------------

const JsonValueSchema: z.ZodType = z.lazy(() =>
  z.union([
    z.number(),
    z.string(),
    z.boolean(),
    z.null(),
    z.array(JsonValueSchema),
    z.record(z.string(), JsonValueSchema),
  ]),
);

const RawEventSchema = z.object({
  id: UuidSchema,
  tick: z.number(),
  event_type: EventTypeSchema,
  agent_id: UuidSchema.nullable(),
  location_id: UuidSchema.nullable(),
  details: JsonValueSchema,
  agent_state_snapshot: AgentStateSnapshotSchema.nullable(),
  world_context: WorldContextSchema,
  created_at: z.string(),
});

export const EventSchema = RawEventSchema;

// ---------------------------------------------------------------------------
// WebSocket tick broadcast schema
// ---------------------------------------------------------------------------

export const TickBroadcastSchema = z.object({
  tick: z.number().int().nonnegative(),
  season: SeasonSchema,
  weather: WeatherSchema,
  agents_alive: z.number().int().nonnegative(),
  deaths_this_tick: z.number().int().nonnegative(),
  actions_resolved: z.number().int().nonnegative(),
});

// ---------------------------------------------------------------------------
// API response schemas
// ---------------------------------------------------------------------------

export const AgentListItemSchema = z.object({
  id: UuidSchema,
  name: z.string(),
  sex: SexSchema.optional(),
  born_at_tick: z.number(),
  died_at_tick: z.number().nullable(),
  cause_of_death: z.string().nullable().optional(),
  generation: z.number(),
  alive: z.boolean(),
  vitals: z
    .object({
      energy: z.number(),
      health: z.number(),
      hunger: z.number(),
      age: z.number(),
    })
    .nullable(),
  location_id: UuidSchema.nullable(),
});

export const AgentsResponseSchema = z.object({
  count: z.number(),
  agents: z.array(AgentListItemSchema),
});

export const AgentDetailResponseSchema = z.object({
  agent: AgentSchema,
  state: AgentStateSchema.nullable(),
});

export const LocationListItemSchema = z.object({
  id: UuidSchema,
  name: z.string(),
  region: z.string(),
  location_type: z.string(),
  capacity: z.number(),
});

export const LocationsResponseSchema = z.object({
  count: z.number(),
  locations: z.array(LocationListItemSchema),
});

export const LocationDetailResponseSchema = z.object({
  location: LocationSchema,
  agents_here: z.array(
    z.object({
      id: UuidSchema,
      name: z.string(),
    }),
  ),
});

export const RoutesResponseSchema = z.object({
  count: z.number(),
  routes: z.array(RouteSchema),
});

export const EventsResponseSchema = z.object({
  count: z.number(),
  events: z.array(EventSchema),
});

// ---------------------------------------------------------------------------
// Operator schemas
// ---------------------------------------------------------------------------

export const InjectedEventTypeSchema = z.enum([
  "natural_disaster",
  "resource_boom",
  "plague",
  "migration",
  "technology_gift",
  "resource_depletion",
]);

export const OperatorStatusSchema = z.object({
  tick: z.number(),
  paused: z.boolean(),
  stop_requested: z.boolean(),
  tick_interval_ms: z.number(),
  elapsed_seconds: z.number(),
  max_ticks: z.number(),
  max_real_time_seconds: z.number(),
  agents_alive: z.number(),
  agents_total: z.number(),
  end_reason: z.string().nullable().optional(),
  started_at: z.string(),
});

export const InjectEventRequestSchema = z.object({
  event_type: InjectedEventTypeSchema,
  target_region: z.string().optional(),
  parameters: z.record(z.string(), JsonValueSchema).optional(),
});

export const OperatorMutationResponseSchema = z.object({
  success: z.boolean(),
  message: z.string(),
});

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

export function parseTickBroadcast(data: unknown) {
  return TickBroadcastSchema.parse(data);
}

export function parseAgentsResponse(data: unknown) {
  return AgentsResponseSchema.parse(data);
}

export function parseAgentDetail(data: unknown) {
  return AgentDetailResponseSchema.parse(data);
}

export function parseLocationsResponse(data: unknown) {
  return LocationsResponseSchema.parse(data);
}

export function parseLocationDetail(data: unknown) {
  return LocationDetailResponseSchema.parse(data);
}

export function parseRoutesResponse(data: unknown) {
  return RoutesResponseSchema.parse(data);
}

export function parseEventsResponse(data: unknown): EventsResponse {
  const raw = EventsResponseSchema.parse(data);
  return raw as unknown as EventsResponse;
}

export function parseWorldSnapshot(data: unknown) {
  return WorldSnapshotSchema.parse(data);
}

export function parseOperatorStatus(data: unknown) {
  return OperatorStatusSchema.parse(data);
}

export function parseOperatorMutationResponse(data: unknown) {
  return OperatorMutationResponseSchema.parse(data);
}

// ---------------------------------------------------------------------------
// Social construct schemas (Phase 6.4)
// ---------------------------------------------------------------------------

export const SocialConstructCategorySchema = z.enum([
  "religion",
  "governance",
  "family",
  "economy",
  "crime_justice",
]);

export const SocialConstructSchema = z.object({
  id: z.string(),
  name: z.string(),
  category: SocialConstructCategorySchema,
  adherent_count: z.number(),
  founded_at_tick: z.number(),
  properties: z.record(z.string(), z.string()),
});

export const BeliefSystemSchema = z.object({
  id: z.string(),
  name: z.string(),
  themes: z.array(z.string()),
  adherent_count: z.number(),
  founded_at_tick: z.number(),
});

export const BeliefEventSchema = z.object({
  tick: z.number(),
  event_type: z.enum(["founded", "schism", "merged", "converted"]),
  belief_system_id: z.string(),
  belief_system_name: z.string(),
  description: z.string(),
  agent_id: z.string().nullable(),
});

export const GovernanceTypeSchema = z.enum([
  "Anarchy",
  "Chieftainship",
  "Council",
  "Monarchy",
  "Democracy",
  "Oligarchy",
  "Theocracy",
]);

export const GovernanceLeaderSchema = z.object({
  agent_id: z.string(),
  agent_name: z.string(),
  role: z.string(),
  since_tick: z.number(),
});

export const GovernanceEventSchema = z.object({
  tick: z.number(),
  event_type: z.enum(["election", "coup", "declaration", "succession", "reform"]),
  description: z.string(),
  agent_id: z.string().nullable(),
});

export const GovernanceInfoSchema = z.object({
  governance_type: GovernanceTypeSchema,
  leaders: z.array(GovernanceLeaderSchema),
  rules: z.array(z.string()),
  stability_score: z.number(),
  recent_events: z.array(GovernanceEventSchema),
});

export const FamilyUnitSchema = z.object({
  id: z.string(),
  name: z.string(),
  members: z.array(z.string()),
  head: z.string(),
  formed_at_tick: z.number(),
});

export const LineageNodeSchema = z.object({
  agent_id: z.string(),
  agent_name: z.string(),
  parent_a: z.string().nullable(),
  parent_b: z.string().nullable(),
  generation: z.number(),
  alive: z.boolean(),
  children: z.array(z.string()),
});

export const FamilyStatsSchema = z.object({
  unit_count: z.number(),
  avg_size: z.number(),
  marriage_count: z.number(),
  divorce_count: z.number(),
  orphan_count: z.number(),
  longest_lineage: z.number(),
  families: z.array(FamilyUnitSchema),
  lineage: z.array(LineageNodeSchema),
});

export const EconomicModelTypeSchema = z.enum([
  "Subsistence",
  "Gift",
  "Barter",
  "Currency",
  "Market",
  "Mixed",
]);

export const MarketLocationSchema = z.object({
  location_id: z.string(),
  location_name: z.string(),
  trade_volume: z.number(),
  primary_resource: ResourceSchema,
});

export const EconomicClassificationSchema = z.object({
  model_type: EconomicModelTypeSchema,
  currency_resource: ResourceSchema.nullable(),
  currency_adoption_pct: z.number(),
  trade_volume: z.number(),
  trade_volume_history: z.array(z.object({ tick: z.number(), volume: z.number() })),
  market_locations: z.array(MarketLocationSchema),
});

export const JusticeTypeSchema = z.enum([
  "None",
  "Vigilante",
  "Elder",
  "Council",
  "Codified",
  "Institutional",
]);

export const CrimeEntrySchema = z.object({
  crime_type: z.string(),
  count: z.number(),
});

export const SerialOffenderSchema = z.object({
  agent_id: z.string(),
  agent_name: z.string(),
  offense_count: z.number(),
  last_offense_tick: z.number(),
});

export const CrimeHotspotSchema = z.object({
  location_id: z.string(),
  location_name: z.string(),
  crime_count: z.number(),
});

export const CrimeStatsSchema = z.object({
  crime_rate: z.number(),
  crime_rate_history: z.array(z.object({ tick: z.number(), rate: z.number() })),
  detection_rate: z.number(),
  punishment_rate: z.number(),
  justice_type: JusticeTypeSchema,
  common_crimes: z.array(CrimeEntrySchema),
  serial_offenders: z.array(SerialOffenderSchema),
  hotspots: z.array(CrimeHotspotSchema),
});

// ---------------------------------------------------------------------------
// Decision record schemas (Phase 9.3 — LLM Decision Viewer)
// ---------------------------------------------------------------------------

export const DecisionSourceSchema = z.enum(["llm", "rule_engine", "night_cycle", "timeout"]);

export const DecisionRecordSchema = z.object({
  agent_id: z.string(),
  tick: z.number(),
  decision_source: DecisionSourceSchema,
  action_type: z.string(),
  action_params: z.record(z.string(), JsonValueSchema).nullable(),
  llm_backend: z.string().nullable(),
  model: z.string().nullable(),
  prompt_tokens: z.number().nullable(),
  completion_tokens: z.number().nullable(),
  cost_usd: z.number().nullable(),
  latency_ms: z.number().nullable(),
  raw_llm_response: z.string().nullable(),
  prompt_sent: z.string().nullable(),
  rule_matched: z.string().nullable(),
  created_at: z.string(),
});

export const DecisionsResponseSchema = z.object({
  count: z.number(),
  decisions: z.array(DecisionRecordSchema),
});

export function parseDecisionsResponse(data: unknown): DecisionsResponse {
  const raw = DecisionsResponseSchema.parse(data);
  return raw as unknown as DecisionsResponse;
}

// ---------------------------------------------------------------------------
// Social API response schemas (Phase 9.7)
// ---------------------------------------------------------------------------

export const BeliefsResponseSchema = z.object({
  belief_systems: z.array(BeliefSystemSchema),
  belief_events: z.array(BeliefEventSchema),
});

export const SocialEconomyResponseSchema = z.object({
  model_type: EconomicModelTypeSchema,
  currency_resource: ResourceSchema.nullable(),
  currency_adoption_pct: z.number(),
  trade_volume: z.number(),
  trade_volume_history: z.array(z.object({ tick: z.number(), volume: z.number() })),
  market_locations: z.array(MarketLocationSchema),
});

// ---------------------------------------------------------------------------
// Social API parsing helpers
// ---------------------------------------------------------------------------

export function parseBeliefsResponse(data: unknown) {
  return BeliefsResponseSchema.parse(data);
}

export function parseGovernanceResponse(data: unknown) {
  return GovernanceInfoSchema.parse(data);
}

export function parseFamiliesResponse(data: unknown) {
  return FamilyStatsSchema.parse(data);
}

export function parseSocialEconomyResponse(data: unknown) {
  return SocialEconomyResponseSchema.parse(data);
}

export function parseCrimeResponse(data: unknown) {
  return CrimeStatsSchema.parse(data);
}
