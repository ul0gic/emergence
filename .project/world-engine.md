# World Engine — Technical Design Document

> **Project:** Emergence
> **Component:** World Engine (core simulation runtime)
> **Status:** Design
> **Date:** February 2026

---

## 1. Overview

The World Engine is the physics engine of the Emergence simulation. It is the single source of truth for what exists, what is possible, and what happens. Every entity, resource, structure, and interaction in the simulation flows through the World Engine.

**The World Engine is NOT an AI.** It is a deterministic rules engine. It does not make decisions. It validates agent actions against world rules, updates state, and emits events. Agents make decisions. The World Engine enforces reality.

Think of it this way:
- **Agents** = the people
- **World Engine** = the laws of physics
- **Dragonfly** = current reality (what exists right now)
- **PostgreSQL** = history (everything that ever happened)

---

## 2. The Tick Cycle

The simulation advances in discrete time steps called **ticks**. One tick = one "world day." Everything that happens in the simulation happens within a tick.

### 2.1 Tick Execution Flow

```
┌──────────────────────────────────────────────────────────────────┐
│                          TICK N                                   │
│                                                                   │
│  PHASE 1: WORLD WAKE                                              │
│     ├─ Advance world clock (tick counter, era tracker)            │
│     ├─ Apply environmental cycles (day/night, seasons, weather)   │
│     ├─ Regenerate resources at resource nodes (configurable rate) │
│     ├─ Decay structures (reduce durability by wear rate)          │
│     ├─ Apply hunger damage to agents (energy -= hunger_rate)      │
│     ├─ Age all agents (age += 1)                                  │
│     ├─ Kill agents with health <= 0 or age > lifespan             │
│     └─ Emit WORLD_TICK_START event                                │
│                                                                   │
│  PHASE 2: PERCEPTION                                              │
│     ├─ For each living agent:                                     │
│     │   ├─ Query world state at agent's location                  │
│     │   ├─ Gather nearby agents, resources, structures            │
│     │   ├─ Assemble perception payload (what agent can "see")     │
│     │   └─ Publish perception to agent's input queue              │
│     └─ All perceptions sent in parallel                           │
│                                                                   │
│  PHASE 3: DECISION (handled by Agent Runners, NOT World Engine)   │
│     ├─ Each agent receives perception payload                     │
│     │   ├─ Agent combines perception + memory + personality       │
│     │   ├─ LLM inference call → returns structured action         │
│     │   └─ Action submitted to World Engine action queue          │
│     └─ Deadline: agents have N seconds to respond or forfeit tick │
│                                                                   │
│  PHASE 4: RESOLUTION                                              │
│     ├─ Collect all submitted actions                              │
│     ├─ Validate each action against world rules                   │
│     ├─ Resolve conflicts (multiple agents want same resource)     │
│     ├─ Execute valid actions (update state in Dragonfly)          │
│     ├─ Reject invalid actions (emit REJECTION event)              │
│     ├─ Process interactions (trades, communication, combat)       │
│     └─ All state changes are atomic per tick                      │
│                                                                   │
│  PHASE 5: PERSIST                                                 │
│     ├─ Flush updated world state from Dragonfly → PostgreSQL      │
│     ├─ Append all events to the event store                       │
│     ├─ Update agent state snapshots                               │
│     ├─ Publish tick summary to Observer Dashboard via pub/sub     │
│     └─ Emit WORLD_TICK_END event                                  │
│                                                                   │
│  PHASE 6: REFLECTION (handled by Agent Runners)                   │
│     ├─ Each agent receives action result (success/failure)        │
│     ├─ Agent updates its own memory and knowledge                 │
│     └─ (This happens async, does not block next tick)             │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 2.2 Tick Timing

| Parameter | Default | Configurable |
|---|---|---|
| Tick interval (real time) | 10 seconds | Yes — can speed up or slow down simulation |
| Agent decision deadline | 8 seconds | Yes — agents that miss deadline forfeit |
| Perception generation | < 500ms target | Performance budget |
| Resolution + Persist | < 1500ms target | Performance budget |

> **Design Note:** The tick interval is the "speed of time." At 10 seconds per tick, 1 real hour = 360 world days ≈ 1 world year. At 1 second per tick, 1 real hour = 3,600 world days ≈ 10 world years. The operator can accelerate time to observe long-term evolution.

### 2.3 Tick Conflict Resolution

When multiple agents act on the same resource or location in the same tick:

1. **First-come, first-served** — Actions are ordered by submission timestamp within the tick window.
2. **Splitting** — If a resource has enough supply for partial fulfillment, split proportionally. (e.g., 3 agents want 5 food each, but only 10 food available → each gets 3, with 1 remaining.)
3. **Bidding** — For contested unique resources (e.g., claiming a specific location), the agent who offers the most resources or has the highest relevant skill wins.
4. **Rejection** — If an action simply cannot be fulfilled, it is rejected. The agent loses nothing but their tick.

---

## 3. World Geography

### 3.1 World Graph

The world is a **directed weighted graph** of locations connected by routes.

```
                    [Mountain Pass]
                    /       \
               (5 ticks)  (4 ticks)
                /             \
    [Forest] --(3 ticks)-- [Riverbank] --(6 ticks)-- [Plains]
                                |
                           (2 ticks)
                                |
                          [Settlement]
                                |
                           (8 ticks, no path)
                                |
                            [Caves]
```

- **Locations (Nodes)** — Discrete places in the world. Each has properties, resources, and a capacity.
- **Routes (Edges)** — Connections between locations. Each has a travel cost in ticks and an optional ACL.
- **Fog of War** — Agents only know about locations they have visited or been told about by other agents. The full map is hidden.

### 3.2 Location Schema

```json
{
  "id": "loc_riverbank",
  "name": "Riverbank",
  "region": "Central Valley",
  "type": "natural",
  "description": "A wide riverbank with fertile soil and fresh water. Trees line the eastern edge.",
  "capacity": 20,
  "current_occupants": ["agent_042", "agent_019"],
  "resources": {
    "water": {"available": 999, "regen_per_tick": 50, "max": 999},
    "wood": {"available": 45, "regen_per_tick": 3, "max": 100},
    "food_berry": {"available": 12, "regen_per_tick": 2, "max": 30},
    "stone": {"available": 8, "regen_per_tick": 0, "max": 8},
    "fish": {"available": 20, "regen_per_tick": 5, "max": 40}
  },
  "structures": ["shelter_007", "firepit_001"],
  "discovered_by": ["agent_042", "agent_019", "agent_003"],
  "routes": [
    {"to": "loc_forest", "cost_ticks": 3, "path_type": "dirt_trail", "acl": null},
    {"to": "loc_settlement", "cost_ticks": 2, "path_type": "worn_path", "acl": null},
    {"to": "loc_plains", "cost_ticks": 6, "path_type": "none", "acl": null}
  ]
}
```

### 3.3 Route Properties

| Property | Description |
|---|---|
| **cost_ticks** | How many ticks it takes to travel this route |
| **path_type** | `none` (wilderness), `dirt_trail`, `worn_path`, `road`, `highway` |
| **acl** | Access Control List — which agents/groups can use this route. `null` = open to all. |
| **durability** | Built paths degrade over time. Agents must maintain them. |
| **capacity** | Max agents that can travel the route per tick (congestion model). |
| **bidirectional** | Whether the route works both ways. Default: true. |

**Path Improvement:** Agents can invest resources to upgrade routes:

```
wilderness (8 ticks) → dirt_trail (5 ticks): costs 10 wood
dirt_trail (5 ticks) → worn_path (3 ticks): costs 20 wood, 10 stone
worn_path (3 ticks) → road (2 ticks): costs 50 wood, 30 stone
road (2 ticks) → highway (1 tick): costs 100 wood, 80 stone, 20 metal
```

### 3.4 Starting World Map

The initial world for Phase 0/1 is small — approximately 8–12 locations across 3 regions:

**Region: Central Valley**
- Riverbank (water, wood, berries, fish, stone)
- Open Field (fertile soil — farmable if agriculture is discovered)
- Forest Edge (abundant wood, some berries, wildlife)

**Region: Highlands**
- Rocky Outcrop (stone, ore, sparse food)
- Mountain Cave (shelter from weather, darkness, minerals)
- Hilltop (visibility — agents here can "see" adjacent regions)

**Region: Coastal Lowlands**
- Beach (sand, driftwood, shellfish, salt)
- Tidal Pools (unique food sources, natural beauty)
- Estuary (rich fishing, reeds for building)

**Undiscovered (require exploration):**
- Deep Forest (rare resources, danger)
- Underground Spring (valuable water source)
- Volcanic Vent (heat source, rare minerals — risk of damage)

> **Expansion:** As population grows, the operator can add new regions or agents can "discover" pre-placed hidden regions through exploration actions.

---

## 4. Resources & Economy

### 4.1 Resource Types

**Tier 0 — Survival (available from tick 0)**

| Resource | Source | Use |
|---|---|---|
| Water | Rivers, springs, rain | Hydration (required daily or energy penalty) |
| Food (berries, fish, roots) | Gathering at resource nodes | Energy restoration |
| Wood | Forests | Building, fuel, tools |
| Stone | Rocky areas | Building, tools |

**Tier 1 — Developed (require discovery or effort)**

| Resource | Source | Use |
|---|---|---|
| Farmed Food | Agriculture (planted + waited) | More reliable food supply |
| Fiber/Reeds | Coastal, riverbank | Clothing, rope, baskets |
| Clay | Riverbanks | Pottery, bricks |
| Animal Hides | Hunting (if agents discover it) | Clothing, shelter improvement |

**Tier 2 — Advanced (require multi-step processes)**

| Resource | Source | Use |
|---|---|---|
| Metal/Ore | Mining at rocky locations | Advanced tools, weapons |
| Smelted Metal | Ore + fire + skill | Superior building, trade goods |
| Medicine | Herb gathering + knowledge | Health restoration |
| Textiles | Fiber + processing | Trade goods, comfort |

**Tier 3 — Complex (require civilization-level coordination)**

| Resource | Source | Use |
|---|---|---|
| Currency Token | Collectively agreed upon / minted | Medium of exchange |
| Written Records | Knowledge + medium (clay, bark) | Persistent knowledge storage |
| Engineered Materials | Multiple resource combinations | Large-scale construction |

### 4.2 The Central Ledger

Every resource unit in the simulation is tracked. Resources are never created from nothing (except via regeneration at nodes) and never destroyed into nothing (except via consumption or decay).

```
LEDGER RULES:
1. Total resources in = total resources out (conservation law)
2. Every transfer is double-entry: one debit, one credit
3. Resource regeneration is an explicit "world → node" credit
4. Resource consumption is an explicit "agent → void" debit
5. The ledger must balance at the end of every tick
6. Imbalances trigger LEDGER_ANOMALY alert to the observer
```

**Ledger Entry Schema:**

```json
{
  "tick": 1205,
  "entry_id": "led_00482910",
  "type": "TRANSFER",
  "from": {"entity": "agent_042", "entity_type": "agent"},
  "to": {"entity": "agent_019", "entity_type": "agent"},
  "resource": "wood",
  "quantity": 5,
  "reason": "TRADE",
  "reference": "trade_00123"
}
```

**Ledger Entry Types:**

| Type | From | To | Example |
|---|---|---|---|
| REGEN | world | resource_node | 3 wood regenerated at Forest |
| GATHER | resource_node | agent | Agent picks 5 berries |
| CONSUME | agent | void | Agent eats 2 food (energy restored) |
| TRANSFER | agent | agent | Trade: 5 wood for 3 food |
| BUILD | agent | structure | 20 wood used to build shelter |
| DECAY | structure | void | Shelter loses 2 durability (material lost) |
| SALVAGE | structure | agent | Agent demolishes shelter, recovers 10 wood |

### 4.3 Agent Wallet

Each agent has an inventory (wallet) of resources. There are no hidden resources. If an agent has it, it's in their wallet.

```json
{
  "agent_id": "agent_042",
  "wallet": {
    "water": 3,
    "food_berry": 5,
    "food_fish": 2,
    "wood": 12,
    "stone": 4,
    "fiber": 0
  },
  "carry_capacity": 50,
  "current_carry": 26
}
```

**Carry Capacity:** Agents have a weight limit. They cannot gather infinitely. This forces decisions about what to carry and creates demand for storage structures.

---

## 5. Structures

Structures are persistent objects built by agents at locations. They provide shelter, storage, production, and social functions.

### 5.1 Structure Schema

```json
{
  "id": "struct_shelter_007",
  "type": "shelter",
  "subtype": "basic_hut",
  "location": "loc_riverbank",
  "builder": "agent_042",
  "owner": "agent_042",
  "built_at_tick": 45,
  "materials_used": {"wood": 20, "stone": 10},
  "durability": {"current": 85, "max": 100, "decay_per_tick": 0.5},
  "capacity": 2,
  "occupants": ["agent_042", "agent_019"],
  "access_list": ["agent_042", "agent_019"],
  "properties": {
    "rest_bonus": 1.5,
    "weather_protection": true,
    "storage_slots": 20
  }
}
```

### 5.2 Structure Types

**Tier 0 — Primitive**

| Structure | Cost | Effect |
|---|---|---|
| Campfire | 3 wood | Warmth, cooking (food efficiency +50%), light |
| Lean-to | 8 wood | Basic shelter, rest bonus x1.2 |
| Basic Hut | 20 wood, 10 stone | Full shelter, rest bonus x1.5, weather protection, storage |

**Tier 1 — Developed**

| Structure | Cost | Effect |
|---|---|---|
| Storage Pit | 10 stone, 5 wood | Extra inventory storage at location |
| Well | 20 stone | Reliable water source (no river needed) |
| Farm Plot | 15 wood (fencing), seeds | Produces food_farmed each tick once planted |
| Workshop | 30 wood, 20 stone | Enables crafting of tools and processed materials |
| Meeting Hall | 50 wood, 30 stone | Enables group decisions for 10+ agents. Governance trigger. |

**Tier 2 — Advanced**

| Structure | Cost | Effect |
|---|---|---|
| Forge | 40 stone, 20 wood, fire | Smelting ore into metal |
| Library | 60 wood, 40 stone | Persistent knowledge storage. Agents can "write" and "read" knowledge. |
| Market | 50 wood, 30 stone | Formal trading with price memory |
| Wall/Fortification | 100 stone, 50 wood | Location defense, access restriction |
| Bridge | 80 wood, 40 stone | New route over obstacle (river, ravine) |

> **Emergence Note:** These tiers are not level-gated. If an agent somehow knows how to build a forge and has the materials, they can build it at tick 5. The tiers describe expected progression, not enforced progression.

### 5.3 Structure Decay & Maintenance

- Every structure loses `decay_per_tick` durability each tick
- When durability reaches 0, the structure collapses
- Collapsed structures return 30% of original materials to the location (salvageable)
- Agents can REPAIR structures by spending resources (cost = proportional to damage)
- Weather events accelerate decay (storms, floods)
- Occupied structures decay slower (inhabited maintenance bonus)

---

## 6. Agent Vitals & Survival

### 6.1 Agent Vital Stats

| Stat | Range | Effect of Depletion |
|---|---|---|
| **Energy** | 0–100 | At 0: agent cannot act (forced rest). Below 20: reduced action options. |
| **Health** | 0–100 | At 0: agent dies. Below 30: reduced energy gain from rest. |
| **Hunger** | 0–100 | Rises each tick. At 100: health damage per tick. Reset by eating. |
| **Age** | 0–lifespan | At lifespan: agent dies. Last 20% of lifespan: reduced energy cap. |

### 6.2 Vital Mechanics Per Tick

```
HUNGER:
  hunger += hunger_rate (default: 5 per tick)
  if hunger >= 100:
    health -= starvation_damage (default: 10 per tick)

ENERGY:
  if agent acted this tick:
    energy -= action_cost (varies by action: gather=10, build=25, move=15, rest=0)
  if agent rested:
    energy += rest_recovery (default: 30, modified by shelter bonus)

EATING:
  if agent eats food:
    hunger -= food_value (berries=20, fish=30, farmed_food=40, cooked_food=50)
    energy += food_energy (berries=5, fish=10, farmed_food=15, cooked_food=20)

AGING:
  age += 1 each tick
  if age > lifespan * 0.8:
    max_energy = 100 * (1 - ((age - lifespan * 0.8) / (lifespan * 0.2)) * 0.5)
    // Energy cap gradually decreases in old age

HEALTH RECOVERY:
  if hunger < 50 AND energy > 50 AND sheltered:
    health += natural_heal_rate (default: 2 per tick)
```

### 6.3 Death

An agent dies when:
- Health reaches 0 (starvation, injury, illness)
- Age exceeds lifespan

**On death:**
1. Agent is removed from active simulation
2. Agent's inventory is dropped at their current location (other agents can scavenge)
3. A DEATH event is emitted with full agent state snapshot
4. Social connections of the dead agent are notified in next perception
5. Agent's structures remain but become unowned (claimable by others)
6. Agent's knowledge can persist if they wrote it to a Library structure

---

## 7. Agent Actions

### 7.1 Action Catalog

These are the valid actions an agent can submit to the World Engine. The World Engine does NOT suggest actions — agents choose from what they know.

**Survival Actions:**

| Action | Energy Cost | Requirements | Effect |
|---|---|---|---|
| `gather` | 10 | At location with resource | Collect resource units (skill-dependent yield) |
| `eat` | 0 | Has food in inventory | Consume food, reduce hunger, restore energy |
| `drink` | 0 | Has water or at water source | Hydrate |
| `rest` | 0 | None (bonus if sheltered) | Restore energy |
| `move` | 15 per tick | Route exists to destination | Begin travel to adjacent location |

**Construction Actions:**

| Action | Energy Cost | Requirements | Effect |
|---|---|---|---|
| `build` | 25 | Has materials, at location, knows blueprint | Create structure |
| `repair` | 15 | At structure, has materials | Restore structure durability |
| `demolish` | 20 | At own structure or unowned | Destroy structure, salvage 30% materials |
| `improve_route` | 30 | At location with route, has materials | Upgrade path between locations |

**Social Actions:**

| Action | Energy Cost | Requirements | Effect |
|---|---|---|---|
| `communicate` | 2 | Another agent at same location | Send message to specific agent |
| `broadcast` | 5 | At location with message board/meeting point | Post message visible to all at location |
| `trade_offer` | 2 | Another agent at same location | Propose a trade |
| `trade_accept` | 0 | Active trade offer directed at agent | Accept trade (ledger transfer) |
| `trade_reject` | 0 | Active trade offer directed at agent | Reject trade |
| `form_group` | 5 | Other willing agents at same location | Create named group with shared goals |
| `teach` | 10 | Another agent at same location, has knowledge they lack | Transfer knowledge to another agent |

**Advanced Actions (available when discovered/unlocked):**

| Action | Energy Cost | Requirements | Effect |
|---|---|---|---|
| `farm_plant` | 20 | Has farm plot + seeds/food to plant | Begin growing cycle |
| `farm_harvest` | 10 | Farm plot with mature crops | Collect farmed food |
| `craft` | 15–30 | Workshop + materials + knowledge | Create tools or processed goods |
| `mine` | 20 | At rocky location, has tools | Extract ore |
| `smelt` | 20 | Forge + ore + fuel | Convert ore to metal |
| `write` | 5 | Library structure + knowledge | Persist knowledge for others |
| `read` | 5 | Library structure with written knowledge | Acquire knowledge from library |
| `claim` | 5 | Unowned structure or unclaimed location | Take ownership |
| `legislate` | 10 | Meeting Hall + group consensus | Create rule/law for the group |
| `enforce` | 15 | Authority granted by group/legislation | Apply consequences for rule violation |
| `reproduce` | 30 | Consenting partner, both with relationship > 0.7, both health > 50 | Spawn child agent |

### 7.2 Action Validation

Every action goes through a validation pipeline:

```
ACTION SUBMITTED
    │
    ▼
[1. SYNTAX CHECK] ── Is this a valid action type with correct parameters?
    │                   NO → REJECT: INVALID_ACTION
    ▼
[2. VITALS CHECK] ── Does the agent have enough energy?
    │                   NO → REJECT: INSUFFICIENT_ENERGY
    ▼
[3. LOCATION CHECK] ── Is the agent at the right location for this action?
    │                    NO → REJECT: WRONG_LOCATION
    ▼
[4. RESOURCE CHECK] ── Does the agent have required materials?
    │                    NO → REJECT: INSUFFICIENT_RESOURCES
    ▼
[5. WORLD STATE CHECK] ── Is the target resource/agent/structure available?
    │                       NO → REJECT: UNAVAILABLE_TARGET
    ▼
[6. SKILL CHECK] ── Does the agent have the knowledge/skill for this action?
    │                 NO → REJECT: UNKNOWN_ACTION
    ▼
[7. CONFLICT CHECK] ── Does this conflict with another agent's action this tick?
    │                    YES → RESOLVE via conflict resolution rules
    ▼
[8. EXECUTE] ── Update world state, debit/credit resources, emit event
```

### 7.3 Action Response

After resolution, each agent receives a result:

```json
{
  "tick": 1205,
  "agent_id": "agent_042",
  "action": "gather",
  "parameters": {"resource": "wood", "location": "loc_forest"},
  "result": "SUCCESS",
  "outcome": {
    "gathered": {"wood": 4},
    "energy_spent": 10,
    "skill_xp": {"gathering": 1}
  },
  "side_effects": [
    "agent_019 watched you gather wood"
  ]
}
```

---

## 8. Knowledge & Discovery

### 8.1 The Knowledge System

Each agent has a knowledge base — a set of things they know. Knowledge determines which actions are available and how effective they are.

**Knowledge is NOT automatically shared.** If agent_042 discovers farming, only agent_042 can farm — unless they TEACH it to others or WRITE it to a library.

### 8.2 Seed Knowledge (by level)

**Level 0 — Blank Slate:**
```json
["exist", "perceive", "move", "basic_communication"]
```
Agents know they exist, can observe surroundings, move between visible locations, and grunt at each other. That's it.

**Level 1 — Primitive:**
```json
["exist", "perceive", "move", "basic_communication", "gather_food", 
 "gather_wood", "gather_stone", "drink_water", "eat", "rest",
 "build_campfire", "build_lean_to", "basic_trade"]
```

**Level 2 — Ancient:**
```json
[...Level 1, "agriculture", "build_hut", "build_storage",
 "pottery", "animal_tracking", "basic_medicine", "barter_system",
 "group_formation", "territorial_claim", "oral_tradition"]
```

**Level 3 — Medieval:**
```json
[...Level 2, "metalworking", "build_forge", "masonry",
 "written_language", "currency_concept", "legislation",
 "organized_labor", "build_wall", "basic_engineering", "bridge_building"]
```

### 8.3 Discovery Mechanics

Agents can discover new knowledge through:

1. **Experimentation** — Agent tries an action that is adjacent to known actions. The World Engine has a discovery probability table.
   ```
   Example: Agent knows "gather_food" and "build_campfire"
   → Chance to discover "cooking" when performing both in same tick = 15%
   → Cooking discovered → cooked_food provides 2x nutrition value
   ```

2. **Observation** — Agent watches another agent perform an action they don't know. Chance to learn based on `curiosity` personality trait.
   ```
   Example: agent_042 gathers wood using a technique agent_019 doesn't know
   → agent_019 is at same location and observes
   → Chance to learn = agent_019.personality.curiosity * 0.3
   ```

3. **Teaching** — Explicit knowledge transfer between agents. High success rate.
   ```
   Cost: 10 energy for teacher, 5 energy for student
   Success rate: 80% + (teacher.skill_level * 5%)
   ```

4. **Reading** — Agent reads from a Library structure. Guaranteed success.

5. **Accidental Discovery** — Small random chance each tick for an agent to discover something adjacent to their existing knowledge. Weighted by curiosity trait.

### 8.4 Discovery Adjacency Map

Not everything can be discovered from nothing. Discoveries have prerequisites:

```
[gather_food] + [observe_seasons] → [agriculture]
[agriculture] + [build_storage] → [food_preservation]
[gather_wood] + [gather_stone] → [basic_tools]
[basic_tools] + [gather_stone] → [mining]
[mining] + [build_campfire] → [smelting]
[smelting] + [basic_tools] → [metalworking]
[basic_communication] + [group_formation] → [oral_tradition]
[oral_tradition] + [clay] → [written_language]
[written_language] + [build_hut] → [library]
[basic_trade] + [group_formation] → [barter_system]
[barter_system] + [written_language] → [currency_concept]
[currency_concept] + [group_formation] → [taxation]
[group_formation] + [territorial_claim] → [governance]
[governance] + [written_language] → [legislation]
[legislation] + [group_formation] → [justice_system]
```

> **This is the tech tree.** But agents don't see it. They don't know what's possible. They just do things, and sometimes something new clicks.

---

## 9. Environment & World Events

### 9.1 Environmental Cycles

| Cycle | Period | Effect |
|---|---|---|
| **Day/Night** | 1 tick = 1 day (day phase + night phase) | Night: reduced perception radius, higher energy cost for actions, rest bonus. |
| **Seasons** | 90 ticks = 1 season (360 ticks = 1 year) | Spring: resource regen boost. Summer: normal. Autumn: harvest bonus, regen slows. Winter: regen minimal, hunger rate increases, exposure risk. |
| **Weather** | Random per tick, weighted by season | Clear, rain (farm bonus, travel penalty), storm (structure damage, travel blocked), drought (water scarcity). |

### 9.2 Operator-Injected World Events

The operator can inject events to test agent response:

| Event | Effect |
|---|---|
| **Natural Disaster** | Destroy structures in a region, scatter resources, kill or injure agents |
| **Resource Boom** | Temporarily double regen rate at specific locations |
| **Plague** | Health damage over time to agents in a region. Tests medicine discovery. |
| **Migration Pressure** | Spawn new agents at world edge (simulates immigration) |
| **Technology Gift** | Grant a random agent one advanced knowledge item. See if it propagates. |
| **Resource Depletion** | Permanently reduce a resource node. Forces migration or innovation. |
| **Contact Event** | Connect two previously isolated regions. Cultural collision. |
| **Internet Unlock** | Give agents access to a shared global communication channel. |

---

## 10. Data Layer

### 10.1 Dragonfly (Hot State)

Dragonfly holds the current tick's complete world state. This is what the World Engine reads and writes during tick execution.

**Key Patterns:**

```
# World clock
world:tick          → 1205
world:era           → "primitive"
world:season        → "autumn"
world:weather       → "rain"

# Agent state (hash per agent)
agent:{id}:vitals   → {"energy": 72, "health": 90, "hunger": 35, "age": 204}
agent:{id}:location → "loc_riverbank"
agent:{id}:wallet   → {"wood": 12, "food_berry": 5, "stone": 4}
agent:{id}:persona  → {"curiosity": 0.8, "aggression": 0.2, ...}
agent:{id}:knowledge → ["gather_food", "build_campfire", "basic_trade", ...]
agent:{id}:memory   → [last N events/interactions as compressed summaries]
agent:{id}:social   → {"agent_019": 0.7, "agent_003": 0.3}
agent:{id}:skills   → {"gathering": 4, "building": 2, "trading": 1}
agent:{id}:goals    → ["find_food", "build_shelter", "explore_north"]

# Location state (hash per location)
loc:{id}:resources  → {"wood": 45, "stone": 8, "food_berry": 12}
loc:{id}:occupants  → ["agent_042", "agent_019"]
loc:{id}:structures → ["struct_shelter_007", "struct_firepit_001"]
loc:{id}:messages   → [recent broadcast messages at this location]

# Structure state
struct:{id}:state   → {full structure JSON}

# Action queue (list — agents push, engine pops)
tick:1205:actions   → [queued action payloads]

# Global indexes
world:agents:alive  → set of living agent IDs
world:agents:dead   → set of dead agent IDs
world:locations     → set of all location IDs
world:structures    → set of all structure IDs
```

### 10.2 PostgreSQL (Persistent State / Event Store)

**Tables:**

```sql
-- Core entity tables
CREATE TABLE agents (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    born_at_tick BIGINT NOT NULL,
    died_at_tick BIGINT,
    cause_of_death TEXT,
    parent_a UUID REFERENCES agents(id),
    parent_b UUID REFERENCES agents(id),
    initial_personality JSONB NOT NULL,
    initial_knowledge JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE locations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    type TEXT NOT NULL,
    description TEXT,
    base_resources JSONB NOT NULL,
    capacity INT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE routes (
    id UUID PRIMARY KEY,
    from_location UUID REFERENCES locations(id),
    to_location UUID REFERENCES locations(id),
    cost_ticks INT NOT NULL,
    path_type TEXT DEFAULT 'none',
    durability INT DEFAULT 100,
    acl JSONB,
    built_by UUID REFERENCES agents(id),
    built_at_tick BIGINT
);

CREATE TABLE structures (
    id UUID PRIMARY KEY,
    type TEXT NOT NULL,
    subtype TEXT,
    location_id UUID REFERENCES locations(id),
    builder UUID REFERENCES agents(id),
    owner UUID REFERENCES agents(id),
    built_at_tick BIGINT NOT NULL,
    destroyed_at_tick BIGINT,
    materials_used JSONB NOT NULL,
    properties JSONB
);

-- The Central Ledger
CREATE TABLE ledger (
    id BIGSERIAL PRIMARY KEY,
    tick BIGINT NOT NULL,
    entry_type TEXT NOT NULL,
    from_entity UUID,
    from_entity_type TEXT,
    to_entity UUID,
    to_entity_type TEXT,
    resource TEXT NOT NULL,
    quantity NUMERIC NOT NULL,
    reason TEXT,
    reference_id TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_ledger_tick ON ledger(tick);
CREATE INDEX idx_ledger_entity ON ledger(from_entity, to_entity);
CREATE INDEX idx_ledger_resource ON ledger(resource);

-- The Event Store (partitioned)
CREATE TABLE events (
    id BIGSERIAL,
    tick BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    agent_id UUID,
    location_id UUID,
    details JSONB NOT NULL,
    agent_state_snapshot JSONB,
    world_context JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
) PARTITION BY RANGE (tick);

-- Create initial partitions (auto-generate more as simulation progresses)
CREATE TABLE events_tick_0_10k PARTITION OF events FOR VALUES FROM (0) TO (10000);
CREATE TABLE events_tick_10k_20k PARTITION OF events FOR VALUES FROM (10000) TO (20000);
CREATE TABLE events_tick_20k_30k PARTITION OF events FOR VALUES FROM (20000) TO (30000);

CREATE INDEX idx_events_tick ON events(tick);
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_agent ON events(agent_id);

-- Discovery milestones
CREATE TABLE discoveries (
    id UUID PRIMARY KEY,
    tick BIGINT NOT NULL,
    agent_id UUID REFERENCES agents(id),
    knowledge_item TEXT NOT NULL,
    method TEXT NOT NULL,
    prerequisites JSONB,
    details JSONB
);

-- Agent state snapshots (periodic, not every tick)
CREATE TABLE agent_snapshots (
    id BIGSERIAL PRIMARY KEY,
    tick BIGINT NOT NULL,
    agent_id UUID REFERENCES agents(id),
    full_state JSONB NOT NULL
);

-- World state snapshots (end of each tick)
CREATE TABLE world_snapshots (
    tick BIGINT PRIMARY KEY,
    population INT,
    total_resources JSONB,
    wealth_distribution JSONB,
    era TEXT,
    season TEXT,
    weather TEXT,
    summary JSONB
);
```

### 10.3 Data Flow Per Tick

```
TICK START
    │
    ├─ World Engine reads current state from Dragonfly
    ├─ Generates perceptions → pushes to agent queues (Dragonfly pub/sub)
    ├─ Agents respond with actions → pushed to action queue (Dragonfly)
    ├─ World Engine pops actions, validates, resolves, executes
    ├─ World Engine writes updated state to Dragonfly
    │
TICK END
    │
    ├─ World Engine flushes state changes → PostgreSQL
    │   ├─ INSERT INTO events (all tick events)
    │   ├─ INSERT INTO ledger (all resource transfers)
    │   ├─ UPDATE agents (state changes)
    │   ├─ INSERT INTO discoveries (if any)
    │   └─ INSERT INTO world_snapshots (tick summary)
    │
    ├─ World Engine publishes tick summary → Dragonfly pub/sub
    │   └─ Observer Dashboard subscribes and updates
    │
    └─ Next tick begins
```

---

## 11. Perception Payload

This is what an agent "sees" at the start of each tick. The World Engine assembles this for each agent individually.

```json
{
  "tick": 1205,
  "time_of_day": "morning",
  "season": "autumn",
  "weather": "rain",
  
  "self": {
    "id": "agent_042",
    "name": "Kora",
    "age": 204,
    "energy": 72,
    "health": 90,
    "hunger": 35,
    "location": "Riverbank",
    "inventory": {
      "wood": 12,
      "food_berry": 5,
      "stone": 4
    },
    "carry_load": "26/50",
    "active_goals": ["build shelter before winter", "find trading partner"],
    "known_skills": ["gathering (lvl 4)", "building (lvl 2)", "trading (lvl 1)"]
  },

  "surroundings": {
    "location_description": "A wide riverbank with fertile soil. Rain patters on the water. Trees sway in the eastern wind.",
    "visible_resources": {
      "wood": "abundant (40+ units)",
      "stone": "scarce (< 10 units)",
      "food_berry": "moderate (10-20 units)",
      "fish": "moderate (15-25 units)",
      "water": "unlimited"
    },
    "structures_here": [
      {"type": "shelter (basic hut)", "owner": "You", "durability": "85%", "occupants": ["You", "Maren"]},
      {"type": "campfire", "owner": "none", "status": "lit"}
    ],
    "agents_here": [
      {"name": "Maren", "relationship": "friendly (0.7)", "activity": "resting in shelter"},
      {"name": "Dax", "relationship": "neutral (0.3)", "activity": "gathering wood"}
    ],
    "messages_here": [
      {"from": "Dax", "tick": 1203, "content": "Anyone want to trade stone for fish?"}
    ]
  },

  "known_routes": [
    {"to": "Forest Edge", "cost": "3 ticks", "path": "dirt trail"},
    {"to": "Settlement", "cost": "2 ticks", "path": "worn path"},
    {"to": "Plains", "cost": "6 ticks", "path": "no path (wilderness)"}
  ],

  "recent_memory": [
    "Tick 1204: You gathered 4 wood at Riverbank.",
    "Tick 1203: Dax asked to trade stone for fish. You ignored it.",
    "Tick 1202: You repaired your shelter (durability 80% → 85%).",
    "Tick 1200: Maren told you she discovered a cave to the north.",
    "Tick 1198: You tried to farm but failed (no agriculture knowledge)."
  ],

  "available_actions": [
    "gather [resource]",
    "eat [food_item]",
    "drink",
    "rest",
    "move [destination]",
    "build [structure_type]",
    "repair [structure]",
    "communicate [agent] [message]",
    "broadcast [message]",
    "trade_offer [agent] [give] [receive]",
    "teach [agent] [knowledge]"
  ],

  "notifications": [
    "It is raining. Travel costs +1 tick. Farm plots gain water bonus.",
    "Winter is approaching in 20 ticks. Prepare food and shelter.",
    "Your shelter durability is at 85%. Consider repairing before winter storms."
  ]
}
```

> **Critical Design Note:** The perception payload is the ONLY information the agent has. There is no "peeking" at world state. If it's not in this payload, the agent doesn't know about it. This is how fog of war works — agents only know what they can see, what they remember, and what others have told them.

---

## 12. Performance Considerations

### 12.1 Scaling Model

| Population | Tick Cycle Target | LLM Calls / Tick | Estimated Cost (API) |
|---|---|---|---|
| 10 agents | 10 sec | 10 | ~$0.01-0.05 / tick |
| 50 agents | 30 sec | 50 | ~$0.05-0.25 / tick |
| 100 agents | 60 sec | 100 | ~$0.10-0.50 / tick |
| 500 agents | 5 min | 500 | ~$0.50-2.50 / tick |

> **Cost Mitigation:** Use local models (Ollama + DeepSeek/Llama) for routine decisions. Reserve API calls (Claude, GPT) for complex social interactions and discovery events.

### 12.2 Parallelism

- Agent decision calls are embarrassingly parallel — fire all LLM calls simultaneously
- World Engine resolution is sequential (must resolve conflicts in order)
- Dragonfly reads/writes are concurrent-safe
- PostgreSQL writes are batched at end of tick

### 12.3 Bottlenecks

| Bottleneck | Mitigation |
|---|---|
| LLM latency per agent | Parallel calls, local models, decision caching for routine actions |
| Dragonfly memory | Agent state compression, periodic snapshot to PG and trim |
| PostgreSQL event volume | Partitioned tables, async writes, batch inserts |
| Perception generation | Pre-compute location state once per tick, not per agent |

---

## 13. Configuration

All world parameters are configurable via a YAML/TOML file loaded at simulation start:

```yaml
# emergence-config.yaml

world:
  name: "Experiment Alpha"
  seed: 42                          # Random seed for reproducibility
  tick_interval_ms: 10000           # Real-time milliseconds per tick
  agent_decision_timeout_ms: 8000
  starting_era: "primitive"
  knowledge_level: 1                # 0=blank, 1=primitive, 2=ancient, etc.
  
time:
  ticks_per_season: 90
  seasons: ["spring", "summer", "autumn", "winter"]
  day_night: true

population:
  initial_agents: 10
  max_agents: 200
  agent_lifespan_ticks: 2500        # ~7 world years
  reproduction_enabled: true
  child_maturity_ticks: 200

economy:
  starting_wallet: {"food_berry": 10, "water": 5, "wood": 3}
  carry_capacity: 50
  hunger_rate: 5
  starvation_damage: 10
  rest_recovery: 30

environment:
  weather_enabled: true
  seasons_enabled: true
  structure_decay_enabled: true
  
discovery:
  accidental_discovery_chance: 0.02   # 2% per tick per agent
  observation_learning_enabled: true
  teaching_success_base: 0.80

infrastructure:
  dragonfly_url: "dragonfly://localhost:6379"
  postgres_url: "postgresql://emergence:emergence@localhost:5432/emergence"
  observer_port: 8080
  event_bus_url: "nats://localhost:4222"

logging:
  level: "info"
  event_store_batch_size: 100
  snapshot_interval_ticks: 100       # Full world snapshot every 100 ticks
```

---

## 14. Implementation: Rust

### 14.1 Why Rust

The World Engine is a financial system disguised as a physics engine. The central ledger with conservation laws, double-entry bookkeeping, and atomic tick resolution demands the same correctness guarantees you'd apply to a bank.

**Key factors:**

1. **Ownership model matches tick architecture** — Each phase has clear ownership of world state. Perception payloads are immutable borrows. Actions transfer ownership back via channels. Rust enforces this at compile time.

2. **Zero-cost abstractions for tick performance** — At 500 agents with 1-second ticks (fast-forward mode), we need deterministic timing. No GC pauses during conflict resolution.

3. **Algebraic types for exhaustive validation** — Action results, rejection reasons, and event types as enums with pattern matching. The compiler guarantees every case is handled.

4. **Compile-time query validation** — `sqlx` checks PostgreSQL queries against the actual schema at compile time. Ledger inserts and event writes are type-checked before the binary exists.

### 14.2 Workspace Structure

```
emergence/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── emergence-core/             # Tick cycle, state machine, orchestration
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── tick.rs           # Tick execution phases
│   │   │   ├── clock.rs          # World clock, era tracking
│   │   │   └── config.rs         # Configuration loading
│   │   └── Cargo.toml
│   │
│   ├── emergence-world/            # Geography, environment, physics
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── location.rs       # Location nodes, capacity, resources
│   │   │   ├── route.rs          # Edges, travel cost, ACLs
│   │   │   ├── structure.rs      # Buildings, decay, maintenance
│   │   │   ├── resource.rs       # Resource types, regeneration
│   │   │   └── environment.rs    # Weather, seasons, day/night
│   │   └── Cargo.toml
│   │
│   ├── emergence-agents/           # Agent state, vitals, actions
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── agent.rs          # Agent identity, personality
│   │   │   ├── vitals.rs         # Energy, health, hunger, age
│   │   │   ├── wallet.rs         # Inventory, carry capacity
│   │   │   ├── knowledge.rs      # Knowledge base, discovery
│   │   │   ├── action.rs         # Action types, validation
│   │   │   └── perception.rs     # Perception payload assembly
│   │   └── Cargo.toml
│   │
│   ├── emergence-ledger/           # Economic core
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ledger.rs         # Central ledger, double-entry
│   │   │   ├── transaction.rs    # Transfer types, validation
│   │   │   └── conservation.rs   # Balance checks, anomaly detection
│   │   └── Cargo.toml
│   │
│   ├── emergence-events/           # Event sourcing
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── event.rs          # Event types, schemas
│   │   │   ├── store.rs          # PostgreSQL event store
│   │   │   └── snapshot.rs       # State snapshots
│   │   └── Cargo.toml
│   │
│   ├── emergence-db/               # Data layer
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── dragonfly.rs      # Hot state operations
│   │   │   ├── postgres.rs       # Persistent state operations
│   │   │   └── migrations/       # SQL migrations
│   │   └── Cargo.toml
│   │
│   ├── emergence-types/            # Shared types + TypeScript generation
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ids.rs            # Type-safe ID wrappers
│   │   │   ├── resources.rs      # Resource enums
│   │   │   ├── actions.rs        # Action request/response types
│   │   │   ├── events.rs         # Event payload types
│   │   │   └── perception.rs     # Perception payload types
│   │   └── Cargo.toml
│   │
│   └── emergence-runner/           # Agent Runner (LLM orchestration)
│       ├── src/
│       │   ├── main.rs           # Runner entry point (separate binary)
│       │   ├── prompt.rs         # Prompt assembly from perception
│       │   ├── llm.rs            # LLM backend abstraction (HTTP clients)
│       │   ├── parse.rs          # Structured action parsing from LLM output
│       │   └── reflect.rs        # Reflection and memory updates
│       └── Cargo.toml
│
├── src/
│   └── main.rs                   # World Engine entry point — starts tick loop
│
└── observer/                     # Dashboard (separate workspace)
    ├── package.json
    ├── src/
    │   └── types/
    │       └── generated.ts      # Generated from emergence-types
    └── ...
```

### 14.3 Lint Configuration

Zero-panic, zero-overflow, zero-unsafe. The compiler is the first line of defense.

```toml
# Cargo.toml (workspace root)

[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Emergence Team"]

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "deny"

[workspace.lints.clippy]
# ============================================
# PANIC PREVENTION
# ============================================
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
unreachable = "deny"
todo = "deny"
unimplemented = "deny"
indexing_slicing = "deny"

# ============================================
# INTEGER SAFETY (critical for ledger)
# ============================================
arithmetic_side_effects = "deny"
cast_possible_truncation = "deny"
cast_sign_loss = "deny"
cast_possible_wrap = "deny"
cast_precision_loss = "deny"

# ============================================
# FLOAT SAFETY (resource calculations)
# ============================================
float_cmp = "deny"
lossy_float_to_int = "deny"

# ============================================
# MEMORY SAFETY
# ============================================
mem_forget = "deny"
rc_buffer = "deny"
rc_mutex = "deny"

# ============================================
# PERFORMANCE
# ============================================
perf = { level = "deny", priority = -1 }
large_enum_variant = "deny"
large_types_passed_by_value = "deny"
needless_collect = "deny"
redundant_clone = "deny"
trivially_copy_pass_by_ref = "deny"
unnecessary_box_returns = "deny"
vec_init_then_push = "deny"

# ============================================
# CODE QUALITY
# ============================================
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }

# Pedantic overrides
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
redundant_pub_crate = "allow"
significant_drop_tightening = "allow"
future_not_send = "allow"

# ============================================
# ASYNC SAFETY
# ============================================
async_yields_async = "deny"
large_futures = "deny"

# ============================================
# ERROR HANDLING
# ============================================
result_large_err = "deny"
map_err_ignore = "deny"
try_err = "deny"

# ============================================
# DOCUMENTATION
# ============================================
missing_const_for_fn = "deny"
doc_markdown = "deny"
```

### 14.4 Core Dependencies

```toml
# Workspace dependencies (inherited by crates)

[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework (observer API)
axum = { version = "0.8", features = ["macros"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }

# Database
sqlx = { version = "0.8", features = [
    "runtime-tokio",
    "postgres", 
    "uuid", 
    "chrono", 
    "migrate",
    "rust_decimal"
] }
rust_decimal = { version = "1", features = ["serde"] }

# Redis/Dragonfly
fred = { version = "9", features = ["tokio-runtime"] }

# Pub/sub
async-nats = "0.38"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Type generation (Rust → TypeScript)
ts-rs = "10"

# IDs
uuid = { version = "1", features = ["v4", "v7", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Config
config = { version = "0.14", features = ["yaml", "toml"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Validation
validator = { version = "0.20", features = ["derive"] }

# HTTP client (LLM API calls from agent runner)
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Prompt templating (agent runner)
minijinja = { version = "2", features = ["loader"] }
```

### 14.5 Type Generation Pipeline

Types are defined once in Rust and flow to TypeScript via `ts-rs`:

```rust
// crates/emergence-types/src/events.rs

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Emitted at the end of each tick with summary data.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub struct TickSummary {
    pub tick: u64,
    pub era: Era,
    pub season: Season,
    pub weather: Weather,
    pub population: PopulationStats,
    pub economy: EconomyStats,
    pub events_count: u32,
    pub discoveries: Vec<Discovery>,
}

/// Agent perception payload — what an agent "sees" each tick.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub struct Perception {
    pub tick: u64,
    pub time_of_day: TimeOfDay,
    pub season: Season,
    pub weather: Weather,
    pub agent: AgentSelf,
    pub surroundings: Surroundings,
    pub known_routes: Vec<KnownRoute>,
    pub recent_memory: Vec<String>,
    pub available_actions: Vec<String>,
    pub notifications: Vec<String>,
}

/// Action submitted by an agent.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub struct ActionRequest {
    pub agent_id: Uuid,
    pub tick: u64,
    pub action: Action,
}

/// Result of action validation and execution.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub struct ActionResult {
    pub tick: u64,
    pub agent_id: Uuid,
    pub action: Action,
    pub result: ActionOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub enum ActionOutcome {
    Success(ActionSuccess),
    Rejected(RejectionReason),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../observer/src/types/generated/")]
pub enum RejectionReason {
    InvalidAction,
    InsufficientEnergy { required: u32, available: u32 },
    WrongLocation { agent_at: Uuid, required: Uuid },
    InsufficientResources { missing: Vec<(Resource, u32)> },
    UnavailableTarget,
    UnknownAction,
    Conflict { resolution: ConflictResolution },
}
```

**Build script generates TypeScript:**

```bash
# scripts/generate-types.sh
cargo test --package emergence-types export_bindings
```

**Generated TypeScript + Zod validation:**

```typescript
// observer/src/types/generated/TickSummary.ts (auto-generated by ts-rs)
export interface TickSummary {
    tick: number;
    era: Era;
    season: Season;
    weather: Weather;
    population: PopulationStats;
    economy: EconomyStats;
    events_count: number;
    discoveries: Array<Discovery>;
}

// observer/src/types/schemas.ts (manual Zod layer)
import { z } from 'zod';
import type { TickSummary } from './generated/TickSummary';

export const TickSummarySchema: z.ZodType<TickSummary> = z.object({
    tick: z.number().int().nonnegative(),
    era: EraSchema,
    season: SeasonSchema,
    weather: WeatherSchema,
    population: PopulationStatsSchema,
    economy: EconomyStatsSchema,
    events_count: z.number().int().nonnegative(),
    discoveries: z.array(DiscoverySchema),
});

// WebSocket message validation
export function parseTickSummary(data: unknown): TickSummary {
    return TickSummarySchema.parse(data);
}
```

This pipeline ensures:
1. Types are defined once in Rust (single source of truth)
2. TypeScript types are always in sync (generated, not manually maintained)
3. Runtime validation catches any drift or malformed messages
4. Zero `any` in the dashboard codebase

---

## 15. Open Design Questions

1. **Should agents see the available actions list, or discover that actions exist?** (Currently: actions listed in perception. Alternative: agents must figure out they can `build` by trying.)
2. **How compressed should memory be?** (Full conversation history is too expensive. Summarized memory loses nuance. What's the right balance?)
3. **Should the World Engine run on a fixed tick or event-driven?** (Fixed tick is simpler. Event-driven is more realistic but harder to synchronize.)
4. **How do we handle agent "sleep"?** (Do agents skip ticks when resting, or do they still perceive but can't act?)
5. **Should resource quantities be visible as exact numbers or fuzzy descriptions?** (Currently: fuzzy — "abundant," "scarce." Alternative: exact numbers.)
6. **What prevents agents from gaming the action system?** (e.g., an agent that always does the mathematically optimal action because it can see the numbers. Fuzzy perception helps but may not be enough.)

---

*This document defines the rules of reality. Everything that happens in Emergence happens because this engine allows it.*
