/**
 * Mock data for development and testing.
 *
 * Provides realistic test data so all dashboard panels render correctly
 * even when no backend is running.
 */
import type {
  AgentDetailResponse,
  AgentListItem,
  Event,
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
