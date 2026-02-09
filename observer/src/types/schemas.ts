/**
 * Zod runtime validation schemas for all data arriving over WebSocket and REST.
 *
 * These schemas validate against the generated TypeScript types. Every message
 * from the wire is parsed through Zod before it touches application state.
 */
import { z } from "zod/v4";

import type { EventsResponse } from "./generated/index.ts";

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

export const AgentSchema = z.object({
  id: UuidSchema,
  name: z.string(),
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
  born_at_tick: z.number(),
  died_at_tick: z.number().nullable(),
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

export const EventsResponseSchema = z.object({
  count: z.number(),
  events: z.array(EventSchema),
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

export function parseEventsResponse(data: unknown): EventsResponse {
  const raw = EventsResponseSchema.parse(data);
  return raw as unknown as EventsResponse;
}

export function parseWorldSnapshot(data: unknown) {
  return WorldSnapshotSchema.parse(data);
}
