/**
 * Mock data for development and testing.
 *
 * Provides realistic test data so all dashboard panels render correctly
 * even when no backend is running.
 */
import type {
  AgentDetailResponse,
  AgentListItem,
  BeliefEvent,
  BeliefSystem,
  CrimeStats,
  EconomicClassification,
  Event,
  FamilyStats,
  GovernanceInfo,
  LocationListItem,
  OperatorStatus,
  TickBroadcast,
  WorldSnapshot,
} from "../types/generated/index.ts";

// ---------------------------------------------------------------------------
// Mock agents
// ---------------------------------------------------------------------------

export const MOCK_AGENTS: AgentListItem[] = [
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    name: "Kora",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 72, health: 90, hunger: 35, age: 204 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a2",
    name: "Maren",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 85, health: 95, hunger: 20, age: 198 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a3",
    name: "Dax",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 45, health: 60, hunger: 70, age: 210 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a4",
    name: "Vela",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 60, health: 80, hunger: 50, age: 190 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c3",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a5",
    name: "Rune",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 92, health: 100, hunger: 10, age: 150 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a6",
    name: "Thane",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 30, health: 45, hunger: 85, age: 240 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c4",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a7",
    name: "Lyra",
    born_at_tick: 100,
    died_at_tick: null,
    generation: 1,
    alive: true,
    vitals: { energy: 80, health: 88, hunger: 25, age: 110 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a8",
    name: "Orin",
    born_at_tick: 0,
    died_at_tick: 180,
    generation: 0,
    alive: false,
    vitals: null,
    location_id: null,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a9",
    name: "Sage",
    born_at_tick: 150,
    died_at_tick: null,
    generation: 1,
    alive: true,
    vitals: { energy: 65, health: 75, hunger: 40, age: 60 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c3",
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc3456789010",
    name: "Ember",
    born_at_tick: 0,
    died_at_tick: null,
    generation: 0,
    alive: true,
    vitals: { energy: 55, health: 70, hunger: 55, age: 200 },
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
  },
];

// ---------------------------------------------------------------------------
// Mock agent detail
// ---------------------------------------------------------------------------

export const MOCK_AGENT_DETAIL: AgentDetailResponse = {
  agent: {
    id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    name: "Kora",
    born_at_tick: 0,
    died_at_tick: null,
    cause_of_death: null,
    parent_a: null,
    parent_b: null,
    generation: 0,
    personality: {
      curiosity: "0.80",
      cooperation: "0.70",
      aggression: "0.20",
      risk_tolerance: "0.60",
      industriousness: "0.85",
      sociability: "0.65",
      honesty: "0.75",
      loyalty: "0.70",
    },
    created_at: "2026-02-01T00:00:00Z",
  },
  state: {
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    energy: 72,
    health: 90,
    hunger: 35,
    age: 204,
    born_at_tick: 0,
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    destination_id: null,
    travel_progress: 0,
    inventory: {
      Wood: 12,
      FoodBerry: 5,
      Stone: 4,
      Water: 3,
    },
    carry_capacity: 50,
    knowledge: [
      "exist",
      "perceive",
      "move",
      "basic_communication",
      "gather_food",
      "gather_wood",
      "gather_stone",
      "drink_water",
      "eat",
      "rest",
      "build_campfire",
      "build_lean_to",
      "basic_trade",
    ],
    skills: {
      gathering: 4,
      building: 2,
      trading: 1,
    },
    skill_xp: {
      gathering: 85,
      building: 30,
      trading: 12,
    },
    goals: ["build shelter before winter", "find trading partner"],
    relationships: {
      "01945c2a-3b4f-7def-8a12-bc34567890a2": "0.70",
      "01945c2a-3b4f-7def-8a12-bc34567890a3": "0.30",
      "01945c2a-3b4f-7def-8a12-bc34567890a5": "0.55",
    },
    memory: [
      {
        tick: 204,
        memory_type: "action",
        summary: "Gathered 4 wood at Riverbank.",
        entities: ["01945c2a-3b4f-7def-8a12-bc34567890c1"],
        emotional_weight: "0.3",
        tier: "Immediate",
      },
      {
        tick: 203,
        memory_type: "observation",
        summary: "Dax asked to trade stone for fish.",
        entities: ["01945c2a-3b4f-7def-8a12-bc34567890a3"],
        emotional_weight: "0.4",
        tier: "ShortTerm",
      },
      {
        tick: 200,
        memory_type: "communication",
        summary: "Maren told me about a cave to the north.",
        entities: ["01945c2a-3b4f-7def-8a12-bc34567890a2"],
        emotional_weight: "0.7",
        tier: "LongTerm",
      },
    ],
  },
};

// ---------------------------------------------------------------------------
// Mock locations
// ---------------------------------------------------------------------------

export const MOCK_LOCATIONS: LocationListItem[] = [
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    name: "Riverbank",
    region: "Central Valley",
    location_type: "natural",
    capacity: 20,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
    name: "Forest Edge",
    region: "Central Valley",
    location_type: "natural",
    capacity: 15,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c3",
    name: "Open Field",
    region: "Central Valley",
    location_type: "natural",
    capacity: 25,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    name: "Rocky Outcrop",
    region: "Highlands",
    location_type: "natural",
    capacity: 10,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c5",
    name: "Mountain Cave",
    region: "Highlands",
    location_type: "natural",
    capacity: 8,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c6",
    name: "Hilltop",
    region: "Highlands",
    location_type: "natural",
    capacity: 12,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c7",
    name: "Beach",
    region: "Coastal Lowlands",
    location_type: "natural",
    capacity: 18,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c8",
    name: "Tidal Pools",
    region: "Coastal Lowlands",
    location_type: "natural",
    capacity: 10,
  },
  {
    id: "01945c2a-3b4f-7def-8a12-bc34567890c9",
    name: "Estuary",
    region: "Coastal Lowlands",
    location_type: "natural",
    capacity: 14,
  },
];

// ---------------------------------------------------------------------------
// Mock routes (for world map edges)
// ---------------------------------------------------------------------------

export interface MockRoute {
  from: string;
  to: string;
  cost: number;
  pathType: string;
}

export const MOCK_ROUTES: MockRoute[] = [
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c2",
    cost: 3,
    pathType: "DirtTrail",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c3",
    cost: 2,
    pathType: "WornPath",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c2",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    cost: 5,
    pathType: "None",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c3",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c7",
    cost: 6,
    pathType: "DirtTrail",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c5",
    cost: 4,
    pathType: "None",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c6",
    cost: 3,
    pathType: "DirtTrail",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c7",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c8",
    cost: 2,
    pathType: "WornPath",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c7",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c9",
    cost: 3,
    pathType: "DirtTrail",
  },
  {
    from: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    to: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    cost: 7,
    pathType: "None",
  },
];

// ---------------------------------------------------------------------------
// Mock events
// ---------------------------------------------------------------------------

export const MOCK_EVENTS: Event[] = [
  {
    id: "e001",
    tick: 210,
    event_type: "ResourceGathered",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: { resource: "Wood", quantity: 4, skill_xp_gained: 1 },
    agent_state_snapshot: {
      energy: 72,
      health: 90,
      hunger: 35,
      age: 204,
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
      inventory_summary: { Wood: 12, FoodBerry: 5 },
    },
    world_context: {
      tick: 210,
      era: "Primitive",
      season: "Autumn",
      weather: "Rain",
      population: 9,
    },
    created_at: "2026-02-08T10:00:00Z",
  },
  {
    id: "e002",
    tick: 209,
    event_type: "TradeCompleted",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a2",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: {
      trade_id: "t001",
      agent_a: "01945c2a-3b4f-7def-8a12-bc34567890a2",
      agent_b: "01945c2a-3b4f-7def-8a12-bc34567890a5",
      gave: { Stone: 3 },
      received: { FoodFish: 2 },
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 209,
      era: "Primitive",
      season: "Autumn",
      weather: "Rain",
      population: 9,
    },
    created_at: "2026-02-08T09:50:00Z",
  },
  {
    id: "e003",
    tick: 208,
    event_type: "KnowledgeDiscovered",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a5",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: {
      knowledge: "cooking",
      method: "experimentation",
      prerequisites: ["gather_food", "build_campfire"],
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 208,
      era: "Primitive",
      season: "Autumn",
      weather: "Clear",
      population: 9,
    },
    created_at: "2026-02-08T09:40:00Z",
  },
  {
    id: "e004",
    tick: 205,
    event_type: "StructureBuilt",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: { structure_type: "BasicHut", materials: { Wood: 20, Stone: 10 } },
    agent_state_snapshot: null,
    world_context: {
      tick: 205,
      era: "Primitive",
      season: "Autumn",
      weather: "Clear",
      population: 10,
    },
    created_at: "2026-02-08T09:20:00Z",
  },
  {
    id: "e005",
    tick: 200,
    event_type: "RelationshipChanged",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: {
      target: "01945c2a-3b4f-7def-8a12-bc34567890a2",
      old_score: "0.5",
      new_score: "0.7",
      cause: "trade",
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 200,
      era: "Primitive",
      season: "Autumn",
      weather: "Clear",
      population: 10,
    },
    created_at: "2026-02-08T09:00:00Z",
  },
  {
    id: "e006",
    tick: 180,
    event_type: "AgentDied",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a8",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c4",
    details: {
      cause: "starvation",
      final_age: 180,
      inventory_dropped: { Stone: 2, Wood: 1 },
      structures_orphaned: [],
    },
    agent_state_snapshot: {
      energy: 0,
      health: 0,
      hunger: 100,
      age: 180,
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c4",
      inventory_summary: { Stone: 2, Wood: 1 },
    },
    world_context: {
      tick: 180,
      era: "Primitive",
      season: "Summer",
      weather: "Drought",
      population: 10,
    },
    created_at: "2026-02-08T08:00:00Z",
  },
  {
    id: "e007",
    tick: 150,
    event_type: "AgentBorn",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a9",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c3",
    details: {
      parent_a: "01945c2a-3b4f-7def-8a12-bc34567890a4",
      parent_b: "01945c2a-3b4f-7def-8a12-bc34567890a3",
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 150,
      era: "Primitive",
      season: "Spring",
      weather: "Clear",
      population: 10,
    },
    created_at: "2026-02-08T07:00:00Z",
  },
  {
    id: "e008",
    tick: 100,
    event_type: "AgentBorn",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a7",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
    details: {
      parent_a: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      parent_b: "01945c2a-3b4f-7def-8a12-bc34567890a2",
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 100,
      era: "Primitive",
      season: "Winter",
      weather: "Snow",
      population: 10,
    },
    created_at: "2026-02-08T06:00:00Z",
  },
  {
    id: "e009",
    tick: 90,
    event_type: "SeasonChanged",
    agent_id: null,
    location_id: null,
    details: { from: "Autumn", to: "Winter" },
    agent_state_snapshot: null,
    world_context: {
      tick: 90,
      era: "Primitive",
      season: "Winter",
      weather: "Snow",
      population: 10,
    },
    created_at: "2026-02-08T05:30:00Z",
  },
  {
    id: "e010",
    tick: 85,
    event_type: "MessageSent",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a2",
    location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
    details: {
      target: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      message: "I found a cave to the north.",
    },
    agent_state_snapshot: null,
    world_context: {
      tick: 85,
      era: "Primitive",
      season: "Autumn",
      weather: "Rain",
      population: 10,
    },
    created_at: "2026-02-08T05:15:00Z",
  },
];

// ---------------------------------------------------------------------------
// Mock world snapshot
// ---------------------------------------------------------------------------

export const MOCK_WORLD_SNAPSHOT: WorldSnapshot = {
  tick: 210,
  era: "Primitive",
  season: "Autumn",
  weather: "Rain",
  population: {
    total_alive: 9,
    total_dead: 1,
    births_this_tick: 0,
    deaths_this_tick: 0,
    average_age: "175.5",
    oldest_agent: "01945c2a-3b4f-7def-8a12-bc34567890a6",
  },
  economy: {
    total_resources: {
      Wood: 320,
      Stone: 150,
      Water: 999,
      FoodBerry: 80,
      FoodFish: 45,
      FoodRoot: 30,
    },
    resources_in_circulation: { Wood: 65, Stone: 28, FoodBerry: 20, FoodFish: 12, Water: 15 },
    resources_at_nodes: {
      Wood: 255,
      Stone: 122,
      Water: 984,
      FoodBerry: 60,
      FoodFish: 33,
      FoodRoot: 30,
    },
    trades_this_tick: 1,
    gini_coefficient: "0.32",
  },
  discoveries: [
    "exist",
    "perceive",
    "move",
    "basic_communication",
    "gather_food",
    "gather_wood",
    "gather_stone",
    "drink_water",
    "eat",
    "rest",
    "build_campfire",
    "build_lean_to",
    "basic_trade",
    "cooking",
  ],
  summary:
    "The community at Riverbank continues to grow. Rune discovered cooking through experimentation. Trade activity is increasing between Maren and Rune.",
};

// ---------------------------------------------------------------------------
// Mock tick history (for charts)
// ---------------------------------------------------------------------------

export function generateMockTickHistory(count: number): TickBroadcast[] {
  const history: TickBroadcast[] = [];
  const seasons: ("Spring" | "Summer" | "Autumn" | "Winter")[] = [
    "Spring",
    "Summer",
    "Autumn",
    "Winter",
  ];
  const weathers: ("Clear" | "Rain" | "Storm" | "Drought" | "Snow")[] = [
    "Clear",
    "Rain",
    "Storm",
    "Drought",
    "Snow",
  ];

  for (let i = 0; i < count; i++) {
    const tick = 210 - i;
    const seasonIdx = Math.floor(tick / 90) % 4;
    // eslint-disable-next-line security/detect-object-injection -- seasonIdx is computed from modulo arithmetic on a fixed-size array, not user input
    const season = seasons[seasonIdx] ?? "Spring";
    const weatherIdx = (tick * 7 + 3) % 5;
    // eslint-disable-next-line security/detect-object-injection -- weatherIdx is computed from modulo arithmetic on a fixed-size array, not user input
    const weather = weathers[weatherIdx] ?? "Clear";
    history.push({
      tick,
      season,
      weather,
      agents_alive: Math.max(8, 10 - Math.floor(i / 50)),
      deaths_this_tick: tick === 180 ? 1 : 0,
      actions_resolved: ((tick * 13 + 7) % 15) + 5,
    });
  }

  return history;
}

// ---------------------------------------------------------------------------
// Mock operator status
// ---------------------------------------------------------------------------

export const MOCK_OPERATOR_STATUS: OperatorStatus = {
  tick: 210,
  elapsed_seconds: 3720,
  max_ticks: 172800,
  max_real_time_seconds: 86400,
  paused: false,
  tick_interval_ms: 500,
  agents_alive: 9,
  agents_dead: 1,
  era: "Primitive",
  season: "Autumn",
  uptime_seconds: 3720,
};

// ---------------------------------------------------------------------------
// Mock social construct data (Phase 6.4 — Social Constructs)
// ---------------------------------------------------------------------------

export const MOCK_BELIEF_SYSTEMS: BeliefSystem[] = [
  {
    id: "bs-001",
    name: "River Watchers",
    themes: ["water", "cycles", "renewal", "fish"],
    adherent_count: 5,
    founded_at_tick: 45,
  },
  {
    id: "bs-002",
    name: "Stone Seekers",
    themes: ["stone", "permanence", "earth", "shelter"],
    adherent_count: 3,
    founded_at_tick: 80,
  },
  {
    id: "bs-003",
    name: "Fire Keepers",
    themes: ["fire", "warmth", "cooking", "transformation"],
    adherent_count: 4,
    founded_at_tick: 120,
  },
];

export const MOCK_BELIEF_EVENTS: BeliefEvent[] = [
  {
    tick: 45,
    event_type: "founded",
    belief_system_id: "bs-001",
    belief_system_name: "River Watchers",
    description:
      "Kora established the River Watchers belief system after observing tidal patterns.",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
  },
  {
    tick: 80,
    event_type: "founded",
    belief_system_id: "bs-002",
    belief_system_name: "Stone Seekers",
    description: "Dax founded Stone Seekers after building the first stone shelter.",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a3",
  },
  {
    tick: 120,
    event_type: "founded",
    belief_system_id: "bs-003",
    belief_system_name: "Fire Keepers",
    description: "Rune established Fire Keepers around communal campfire rituals.",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a5",
  },
  {
    tick: 160,
    event_type: "converted",
    belief_system_id: "bs-001",
    belief_system_name: "River Watchers",
    description: "Maren converted to River Watchers after receiving help at the Riverbank.",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a2",
  },
  {
    tick: 195,
    event_type: "schism",
    belief_system_id: "bs-003",
    belief_system_name: "Fire Keepers",
    description: "Thane rejected Fire Keeper teachings, causing a minor schism.",
    agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a6",
  },
];

export const MOCK_GOVERNANCE: GovernanceInfo = {
  governance_type: "Chieftainship",
  leaders: [
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      agent_name: "Kora",
      role: "Chief",
      since_tick: 130,
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a5",
      agent_name: "Rune",
      role: "Advisor",
      since_tick: 170,
    },
  ],
  rules: [
    "No stealing from communal stores",
    "Share food during drought",
    "Disputes settled by Chief",
  ],
  stability_score: 0.72,
  recent_events: [
    {
      tick: 200,
      event_type: "declaration",
      description: "Kora declared drought sharing rule after observing hoarding behavior.",
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    },
    {
      tick: 185,
      event_type: "succession",
      description: "Rune appointed as advisor after demonstrating cooking knowledge.",
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a5",
    },
    {
      tick: 170,
      event_type: "election",
      description: "Group consensus chose Kora as chief based on building contributions.",
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
    },
  ],
};

export const MOCK_FAMILY_STATS: FamilyStats = {
  unit_count: 3,
  avg_size: 2.7,
  marriage_count: 2,
  divorce_count: 0,
  orphan_count: 0,
  longest_lineage: 2,
  families: [
    {
      id: "fam-001",
      name: "Kora-Maren",
      members: [
        "01945c2a-3b4f-7def-8a12-bc34567890a1",
        "01945c2a-3b4f-7def-8a12-bc34567890a2",
        "01945c2a-3b4f-7def-8a12-bc34567890a7",
      ],
      head: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      formed_at_tick: 60,
    },
    {
      id: "fam-002",
      name: "Dax-Vela",
      members: [
        "01945c2a-3b4f-7def-8a12-bc34567890a3",
        "01945c2a-3b4f-7def-8a12-bc34567890a4",
        "01945c2a-3b4f-7def-8a12-bc34567890a9",
      ],
      head: "01945c2a-3b4f-7def-8a12-bc34567890a4",
      formed_at_tick: 90,
    },
    {
      id: "fam-003",
      name: "Rune-Ember",
      members: ["01945c2a-3b4f-7def-8a12-bc34567890a5", "01945c2a-3b4f-7def-8a12-bc3456789010"],
      head: "01945c2a-3b4f-7def-8a12-bc34567890a5",
      formed_at_tick: 140,
    },
  ],
  lineage: [
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      agent_name: "Kora",
      parent_a: null,
      parent_b: null,
      generation: 0,
      alive: true,
      children: ["01945c2a-3b4f-7def-8a12-bc34567890a7"],
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a2",
      agent_name: "Maren",
      parent_a: null,
      parent_b: null,
      generation: 0,
      alive: true,
      children: ["01945c2a-3b4f-7def-8a12-bc34567890a7"],
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a7",
      agent_name: "Lyra",
      parent_a: "01945c2a-3b4f-7def-8a12-bc34567890a1",
      parent_b: "01945c2a-3b4f-7def-8a12-bc34567890a2",
      generation: 1,
      alive: true,
      children: [],
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a3",
      agent_name: "Dax",
      parent_a: null,
      parent_b: null,
      generation: 0,
      alive: true,
      children: ["01945c2a-3b4f-7def-8a12-bc34567890a9"],
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a4",
      agent_name: "Vela",
      parent_a: null,
      parent_b: null,
      generation: 0,
      alive: true,
      children: ["01945c2a-3b4f-7def-8a12-bc34567890a9"],
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a9",
      agent_name: "Sage",
      parent_a: "01945c2a-3b4f-7def-8a12-bc34567890a4",
      parent_b: "01945c2a-3b4f-7def-8a12-bc34567890a3",
      generation: 1,
      alive: true,
      children: [],
    },
  ],
};

export const MOCK_ECONOMIC_CLASSIFICATION: EconomicClassification = {
  model_type: "Barter",
  currency_resource: null,
  currency_adoption_pct: 0,
  trade_volume: 47,
  trade_volume_history: [
    { tick: 50, volume: 2 },
    { tick: 70, volume: 5 },
    { tick: 90, volume: 4 },
    { tick: 110, volume: 8 },
    { tick: 130, volume: 6 },
    { tick: 150, volume: 12 },
    { tick: 170, volume: 9 },
    { tick: 190, volume: 15 },
    { tick: 210, volume: 18 },
  ],
  market_locations: [
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
      location_name: "Riverbank",
      trade_volume: 28,
      primary_resource: "FoodFish",
    },
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c3",
      location_name: "Open Field",
      trade_volume: 12,
      primary_resource: "FoodBerry",
    },
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
      location_name: "Forest Edge",
      trade_volume: 7,
      primary_resource: "Wood",
    },
  ],
};

export const MOCK_CRIME_STATS: CrimeStats = {
  crime_rate: 0.08,
  crime_rate_history: [
    { tick: 50, rate: 0.0 },
    { tick: 70, rate: 0.02 },
    { tick: 90, rate: 0.0 },
    { tick: 110, rate: 0.05 },
    { tick: 130, rate: 0.03 },
    { tick: 150, rate: 0.06 },
    { tick: 170, rate: 0.1 },
    { tick: 190, rate: 0.12 },
    { tick: 210, rate: 0.08 },
  ],
  detection_rate: 0.65,
  punishment_rate: 0.4,
  justice_type: "Elder",
  common_crimes: [
    { crime_type: "Resource theft", count: 5 },
    { crime_type: "Trespassing", count: 3 },
    { crime_type: "Food hoarding", count: 2 },
    { crime_type: "Trade fraud", count: 1 },
  ],
  serial_offenders: [
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a6",
      agent_name: "Thane",
      offense_count: 4,
      last_offense_tick: 195,
    },
    {
      agent_id: "01945c2a-3b4f-7def-8a12-bc34567890a3",
      agent_name: "Dax",
      offense_count: 2,
      last_offense_tick: 170,
    },
  ],
  hotspots: [
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c4",
      location_name: "Rocky Outcrop",
      crime_count: 4,
    },
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c2",
      location_name: "Forest Edge",
      crime_count: 3,
    },
    {
      location_id: "01945c2a-3b4f-7def-8a12-bc34567890c1",
      location_name: "Riverbank",
      crime_count: 2,
    },
  ],
};
