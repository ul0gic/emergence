# Data Schemas — Technical Specification

> **Project:** Emergence
> **Component:** Data Layer
> **Status:** Design
> **Date:** February 2026

---

## 1. Overview

This document defines the canonical data structures for the Genesis simulation. All types are defined here once and generate downstream to:
- Rust structs (World Engine, source of truth)
- TypeScript interfaces (Observer Dashboard, via ts-rs)
- Zod schemas (Runtime validation)
- PostgreSQL tables (Persistent storage)
- Dragonfly keys (Hot state)

**Single source of truth principle:** If a field exists, it is defined here. If it is not defined here, it does not exist.

---

## 2. Identifier Types

All entities use UUID v7 for identifiers (time-ordered for efficient indexing).

| ID Type | Format | Example |
|---|---|---|
| **AgentId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc34567890ab` |
| **LocationId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc34567890cd` |
| **StructureId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc34567890ef` |
| **RouteId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc3456789012` |
| **EventId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc3456789034` |
| **TradeId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc3456789056` |
| **GroupId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc3456789078` |
| **LedgerEntryId** | UUID v7 | `01945c2a-3b4f-7def-8a12-bc345678909a` |

---

## 3. Enumerations

### 3.1 Resource Types

| Resource | Tier | Category |
|---|---|---|
| **Water** | 0 | Survival |
| **FoodBerry** | 0 | Survival |
| **FoodFish** | 0 | Survival |
| **FoodRoot** | 0 | Survival |
| **FoodMeat** | 1 | Survival |
| **FoodFarmed** | 1 | Survival |
| **FoodCooked** | 1 | Survival |
| **Wood** | 0 | Material |
| **Stone** | 0 | Material |
| **Fiber** | 1 | Material |
| **Clay** | 1 | Material |
| **Hide** | 1 | Material |
| **Ore** | 2 | Material |
| **Metal** | 2 | Material |
| **Medicine** | 2 | Consumable |
| **Tool** | 1 | Equipment |
| **ToolAdvanced** | 2 | Equipment |
| **CurrencyToken** | 3 | Abstract |
| **WrittenRecord** | 3 | Abstract |

### 3.2 Structure Types

| Structure | Tier | Category |
|---|---|---|
| **Campfire** | 0 | Utility |
| **LeanTo** | 0 | Shelter |
| **BasicHut** | 0 | Shelter |
| **StoragePit** | 1 | Storage |
| **Well** | 1 | Production |
| **FarmPlot** | 1 | Production |
| **Workshop** | 1 | Production |
| **MeetingHall** | 1 | Social |
| **Forge** | 2 | Production |
| **Library** | 2 | Knowledge |
| **Market** | 2 | Economic |
| **Wall** | 2 | Defense |
| **Bridge** | 2 | Infrastructure |

### 3.3 Action Types

| Action | Category |
|---|---|
| **Gather** | Survival |
| **Eat** | Survival |
| **Drink** | Survival |
| **Rest** | Survival |
| **Move** | Movement |
| **Build** | Construction |
| **Repair** | Construction |
| **Demolish** | Construction |
| **ImproveRoute** | Construction |
| **Communicate** | Social |
| **Broadcast** | Social |
| **TradeOffer** | Social |
| **TradeAccept** | Social |
| **TradeReject** | Social |
| **FormGroup** | Social |
| **Teach** | Social |
| **FarmPlant** | Advanced |
| **FarmHarvest** | Advanced |
| **Craft** | Advanced |
| **Mine** | Advanced |
| **Smelt** | Advanced |
| **Write** | Advanced |
| **Read** | Advanced |
| **Claim** | Advanced |
| **Legislate** | Advanced |
| **Enforce** | Advanced |
| **Reproduce** | Advanced |
| **Steal** | Conflict |
| **Attack** | Conflict |
| **Propose** | Diplomacy |
| **Vote** | Diplomacy |
| **Marry** | Social |
| **Divorce** | Social |
| **Conspire** | Social |
| **Pray** | Cultural |
| **NoAction** | System |

### 3.4 Event Types

| Event | Category | Description |
|---|---|---|
| **TickStart** | System | Beginning of tick |
| **TickEnd** | System | End of tick |
| **AgentBorn** | Lifecycle | New agent created |
| **AgentDied** | Lifecycle | Agent death |
| **ActionSubmitted** | Action | Agent submitted action |
| **ActionSucceeded** | Action | Action completed successfully |
| **ActionRejected** | Action | Action failed validation |
| **ResourceGathered** | Economy | Agent collected resources |
| **ResourceConsumed** | Economy | Agent used resources |
| **TradeCompleted** | Economy | Two agents exchanged resources |
| **TradeFailed** | Economy | Trade rejected or invalid |
| **StructureBuilt** | World | New structure created |
| **StructureDestroyed** | World | Structure collapsed or demolished |
| **StructureRepaired** | World | Structure durability restored |
| **RouteImproved** | World | Path upgraded |
| **LocationDiscovered** | World | Agent found new location |
| **KnowledgeDiscovered** | Knowledge | Agent learned something new |
| **KnowledgeTaught** | Knowledge | Knowledge transferred between agents |
| **MessageSent** | Social | Agent communicated |
| **GroupFormed** | Social | New group created |
| **RelationshipChanged** | Social | Relationship score updated |
| **WeatherChanged** | Environment | Weather shifted |
| **SeasonChanged** | Environment | Season transitioned |
| **LedgerAnomaly** | System | Conservation law violated (alert) |
| **TheftOccurred** | Conflict | Agent successfully stole from another agent |
| **TheftFailed** | Conflict | Theft attempt was detected and prevented |
| **CombatInitiated** | Conflict | Physical confrontation started between agents |
| **CombatResolved** | Conflict | Combat concluded with outcome |
| **DeceptionCommitted** | Social | Agent deliberately lied or misled another agent |
| **DeceptionDiscovered** | Social | A prior deception was uncovered |
| **AllianceFormed** | Diplomacy | Two or more agents/groups formed an alliance |
| **AllianceBroken** | Diplomacy | An existing alliance was dissolved |
| **WarDeclared** | Diplomacy | Formal conflict declared between groups |
| **TreatyNegotiated** | Diplomacy | Peace or trade agreement reached |
| **SocialConstructFormed** | Emergence | New emergent social structure detected |
| **SocialConstructDisbanded** | Emergence | Social structure dissolved |
| **ReputationChanged** | Social | Agent's observable reputation shifted |
| **OperatorAction** | System | External operator intervention recorded |
| **SimulationPaused** | System | Simulation tick loop halted by operator |
| **SimulationResumed** | System | Simulation tick loop resumed by operator |
| **SimulationEnded** | System | Simulation terminated (time limit, extinction, or manual) |

### 3.5 Rejection Reasons

| Reason | Description |
|---|---|
| **InvalidAction** | Action type not recognized |
| **InsufficientEnergy** | Agent lacks energy for action |
| **WrongLocation** | Agent not at required location |
| **InsufficientResources** | Agent lacks required materials |
| **UnavailableTarget** | Target resource/agent/structure not available |
| **UnknownAction** | Agent lacks knowledge for action |
| **ConflictLost** | Another agent won contested resource |
| **CapacityExceeded** | Would exceed carry capacity |
| **InvalidTarget** | Target agent/structure does not exist |
| **PermissionDenied** | ACL prevents action |
| **Timeout** | Agent missed decision deadline |

### 3.6 Seasons

| Season | Resource Effect | Hunger Effect |
|---|---|---|
| **Spring** | Regeneration +25% | Normal |
| **Summer** | Normal | Normal |
| **Autumn** | Harvest +50%, Regen -25% | Normal |
| **Winter** | Regeneration -75% | +50% |

### 3.7 Weather

| Weather | Travel Effect | Structure Effect | Farm Effect |
|---|---|---|---|
| **Clear** | Normal | Normal | Normal |
| **Rain** | +1 tick cost | Normal | +25% growth |
| **Storm** | Travel blocked | Decay +100% | Damage risk |
| **Drought** | Normal | Normal | Growth stopped |
| **Snow** | +2 tick cost | Decay +50% | Growth stopped |

### 3.8 Path Types

| Path Type | Base Tick Cost | Description |
|---|---|---|
| **None** | 8 | Wilderness, no path |
| **DirtTrail** | 5 | Basic cleared path |
| **WornPath** | 3 | Established foot traffic |
| **Road** | 2 | Constructed road |
| **Highway** | 1 | Major infrastructure |

### 3.9 Time of Day

| Time | Perception Effect | Energy Effect |
|---|---|---|
| **Dawn** | Normal | Rest bonus ending |
| **Morning** | Normal | Normal |
| **Afternoon** | Normal | Normal |
| **Dusk** | Normal | Normal |
| **Night** | Reduced radius | Action cost +25%, Rest bonus +50% |

### 3.10 Eras

| Era | Trigger |
|---|---|
| **Primitive** | Starting era |
| **Tribal** | Group formation emerged |
| **Agricultural** | Farming discovered |
| **Settlement** | Permanent structures established |
| **Bronze** | Metalworking discovered |
| **Iron** | Advanced metalworking |
| **Classical** | Written language and governance |
| **Medieval** | Complex institutions |
| **Industrial** | Manufacturing (if reached) |
| **Modern** | Full technology (if reached) |

---

## 4. Core Entity Schemas

### 4.1 Agent

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | AgentId | No | Unique identifier |
| **name** | String | No | Display name |
| **born_at_tick** | Integer | No | Creation tick |
| **died_at_tick** | Integer | Yes | Death tick (null if alive) |
| **cause_of_death** | String | Yes | How agent died |
| **parent_a** | AgentId | Yes | First parent |
| **parent_b** | AgentId | Yes | Second parent |
| **generation** | Integer | No | Generation number (0 = seed) |
| **personality** | Personality | No | Immutable personality vector |
| **created_at** | Timestamp | No | Real-world creation time |

### 4.2 Personality

| Field | Type | Range | Description |
|---|---|---|---|
| **curiosity** | Float | 0.0–1.0 | Exploration tendency |
| **cooperation** | Float | 0.0–1.0 | Collaboration preference |
| **aggression** | Float | 0.0–1.0 | Conflict tendency |
| **risk_tolerance** | Float | 0.0–1.0 | Uncertainty acceptance |
| **industriousness** | Float | 0.0–1.0 | Work preference |
| **sociability** | Float | 0.0–1.0 | Interaction desire |
| **honesty** | Float | 0.0–1.0 | Truthfulness |
| **loyalty** | Float | 0.0–1.0 | Commitment strength |

### 4.3 AgentState (Mutable)

| Field | Type | Description |
|---|---|---|
| **agent_id** | AgentId | Reference to agent |
| **energy** | Integer (0–100) | Current energy |
| **health** | Integer (0–100) | Current health |
| **hunger** | Integer (0–100) | Current hunger level |
| **age** | Integer | Current age in ticks |
| **location_id** | LocationId | Current location |
| **destination_id** | LocationId (nullable) | Travel destination |
| **travel_progress** | Integer | Ticks until arrival |
| **inventory** | Map<Resource, Integer> | Carried resources |
| **carry_capacity** | Integer | Max carry weight |
| **knowledge** | Set<String> | Known concepts |
| **skills** | Map<String, Integer> | Skill levels |
| **skill_xp** | Map<String, Integer> | Experience points |
| **goals** | List<String> | Active goals (max 5) |
| **relationships** | Map<AgentId, Float> | Social graph |
| **memory** | List<MemoryEntry> | Agent memories |

### 4.4 Location

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | LocationId | No | Unique identifier |
| **name** | String | No | Display name |
| **region** | String | No | Region name |
| **location_type** | String | No | Category (natural, settlement, etc.) |
| **description** | String | No | Narrative description |
| **capacity** | Integer | No | Max agents |
| **base_resources** | Map<Resource, ResourceNode> | No | Resource availability |
| **discovered_by** | Set<AgentId> | No | Agents who know this location |
| **created_at** | Timestamp | No | Real-world creation time |

### 4.5 ResourceNode

| Field | Type | Description |
|---|---|---|
| **resource** | Resource | Resource type |
| **available** | Integer | Current quantity |
| **regen_per_tick** | Integer | Regeneration rate |
| **max_capacity** | Integer | Maximum quantity |

### 4.6 Route

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | RouteId | No | Unique identifier |
| **from_location** | LocationId | No | Origin |
| **to_location** | LocationId | No | Destination |
| **cost_ticks** | Integer | No | Travel time |
| **path_type** | PathType | No | Road quality |
| **durability** | Integer (0–100) | No | Current condition |
| **max_durability** | Integer | No | Maximum condition |
| **decay_per_tick** | Float | No | Degradation rate |
| **acl** | AccessControlList (nullable) | Yes | Access restrictions |
| **bidirectional** | Boolean | No | Works both ways |
| **built_by** | AgentId | Yes | Constructor (null if natural) |
| **built_at_tick** | Integer | Yes | Construction tick |

### 4.7 AccessControlList

| Field | Type | Description |
|---|---|---|
| **allowed_agents** | Set<AgentId> | Explicitly allowed agents |
| **allowed_groups** | Set<GroupId> | Explicitly allowed groups |
| **denied_agents** | Set<AgentId> | Explicitly denied agents |
| **public** | Boolean | Open to all if true |
| **toll_cost** | Map<Resource, Integer> (nullable) | Required payment |

### 4.8 Structure

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | StructureId | No | Unique identifier |
| **structure_type** | StructureType | No | Category |
| **subtype** | String | Yes | Specific variant |
| **location_id** | LocationId | No | Where it exists |
| **builder** | AgentId | No | Who built it |
| **owner** | AgentId | Yes | Current owner |
| **built_at_tick** | Integer | No | Construction tick |
| **destroyed_at_tick** | Integer | Yes | Destruction tick |
| **materials_used** | Map<Resource, Integer> | No | Construction cost |
| **durability** | Integer (0–100) | No | Current condition |
| **max_durability** | Integer | No | Maximum condition |
| **decay_per_tick** | Float | No | Degradation rate |
| **capacity** | Integer | No | Occupant limit |
| **occupants** | Set<AgentId> | No | Current occupants |
| **access_list** | AccessControlList | Yes | Access restrictions |
| **properties** | StructureProperties | No | Type-specific properties |

### 4.9 StructureProperties

| Field | Type | Description |
|---|---|---|
| **rest_bonus** | Float | Multiplier for rest recovery |
| **weather_protection** | Boolean | Blocks weather effects |
| **storage_slots** | Integer | Additional inventory space |
| **production_type** | Resource (nullable) | What it produces |
| **production_rate** | Integer | Units per tick |

### 4.10 MemoryEntry

| Field | Type | Description |
|---|---|---|
| **tick** | Integer | When it occurred |
| **memory_type** | String | Category |
| **summary** | String | Human-readable description |
| **entities** | List<UUID> | Related entities |
| **emotional_weight** | Float (0.0–1.0) | Retention importance |
| **tier** | String | Immediate, ShortTerm, or LongTerm |

---

## 5. Event Schemas

### 5.1 Base Event

All events share these fields:

| Field | Type | Description |
|---|---|---|
| **id** | EventId | Unique identifier |
| **tick** | Integer | When it occurred |
| **event_type** | EventType | Category |
| **agent_id** | AgentId (nullable) | Primary agent involved |
| **location_id** | LocationId (nullable) | Where it occurred |
| **details** | EventDetails | Type-specific payload |
| **agent_state_snapshot** | AgentStateSnapshot (nullable) | Agent state at time of event |
| **world_context** | WorldContext | World state context |
| **created_at** | Timestamp | Real-world timestamp |

### 5.2 WorldContext

| Field | Type | Description |
|---|---|---|
| **tick** | Integer | Current tick |
| **era** | Era | Current era |
| **season** | Season | Current season |
| **weather** | Weather | Current weather |
| **population** | Integer | Living agent count |

### 5.3 AgentStateSnapshot

| Field | Type | Description |
|---|---|---|
| **energy** | Integer | Energy at event time |
| **health** | Integer | Health at event time |
| **hunger** | Integer | Hunger at event time |
| **age** | Integer | Age at event time |
| **location_id** | LocationId | Location at event time |
| **inventory_summary** | Map<Resource, Integer> | Inventory at event time |

### 5.4 Event Details by Type

**ActionSucceeded:**
| Field | Type |
|---|---|
| action_type | ActionType |
| parameters | Map<String, Any> |
| outcome | ActionOutcome |

**ActionRejected:**
| Field | Type |
|---|---|
| action_type | ActionType |
| parameters | Map<String, Any> |
| reason | RejectionReason |
| reason_details | Map<String, Any> |

**ResourceGathered:**
| Field | Type |
|---|---|
| resource | Resource |
| quantity | Integer |
| location_id | LocationId |
| skill_xp_gained | Integer |

**TradeCompleted:**
| Field | Type |
|---|---|
| trade_id | TradeId |
| agent_a | AgentId |
| agent_b | AgentId |
| gave | Map<Resource, Integer> |
| received | Map<Resource, Integer> |

**KnowledgeDiscovered:**
| Field | Type |
|---|---|
| knowledge | String |
| method | String (experimentation, observation, accidental) |
| prerequisites | List<String> |

**AgentDied:**
| Field | Type |
|---|---|
| cause | String (starvation, old_age, injury) |
| final_age | Integer |
| inventory_dropped | Map<Resource, Integer> |
| structures_orphaned | List<StructureId> |

---

## 6. Ledger Schemas

### 6.1 LedgerEntry

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | LedgerEntryId | No | Unique identifier |
| **tick** | Integer | No | When transfer occurred |
| **entry_type** | LedgerEntryType | No | Category |
| **from_entity** | UUID | Yes | Source entity |
| **from_entity_type** | String | Yes | Source type (agent, location, structure, world, void) |
| **to_entity** | UUID | Yes | Destination entity |
| **to_entity_type** | String | Yes | Destination type |
| **resource** | Resource | No | What transferred |
| **quantity** | Integer | No | How much |
| **reason** | String | No | Why (GATHER, TRADE, BUILD, etc.) |
| **reference_id** | UUID | Yes | Related entity (trade_id, structure_id, etc.) |
| **created_at** | Timestamp | No | Real-world timestamp |

### 6.2 LedgerEntryType

| Type | From | To | Description |
|---|---|---|---|
| **Regeneration** | World | Location | Resource respawned |
| **Gather** | Location | Agent | Agent collected resource |
| **Consume** | Agent | Void | Agent used resource (eating, fuel) |
| **Transfer** | Agent | Agent | Trade or gift |
| **Build** | Agent | Structure | Construction material |
| **Salvage** | Structure | Agent | Demolition recovery |
| **Decay** | Structure | Void | Degradation loss |
| **Drop** | Agent | Location | Death inventory drop |
| **Pickup** | Location | Agent | Scavenging dropped items |

---

## 7. Action Schemas

### 7.1 ActionRequest

| Field | Type | Description |
|---|---|---|
| **agent_id** | AgentId | Who is acting |
| **tick** | Integer | Current tick |
| **action_type** | ActionType | What action |
| **parameters** | ActionParameters | Action-specific data |
| **submitted_at** | Timestamp | Submission time |

### 7.2 ActionParameters by Type

**Gather:**
| Field | Type |
|---|---|
| resource | Resource |

**Eat:**
| Field | Type |
|---|---|
| food_type | Resource |

**Move:**
| Field | Type |
|---|---|
| destination | LocationId |

**Build:**
| Field | Type |
|---|---|
| structure_type | StructureType |

**Repair:**
| Field | Type |
|---|---|
| structure_id | StructureId |

**Communicate:**
| Field | Type |
|---|---|
| target_agent | AgentId |
| message | String (max 500 chars) |

**Broadcast:**
| Field | Type |
|---|---|
| message | String (max 500 chars) |

**TradeOffer:**
| Field | Type |
|---|---|
| target_agent | AgentId |
| offer | Map<Resource, Integer> |
| request | Map<Resource, Integer> |

**TradeAccept / TradeReject:**
| Field | Type |
|---|---|
| trade_id | TradeId |

**Teach:**
| Field | Type |
|---|---|
| target_agent | AgentId |
| knowledge | String |

**Reproduce:**
| Field | Type |
|---|---|
| partner_agent | AgentId |

### 7.3 ActionResult

| Field | Type | Description |
|---|---|---|
| **tick** | Integer | Tick of action |
| **agent_id** | AgentId | Who acted |
| **action_type** | ActionType | What was attempted |
| **success** | Boolean | Did it work |
| **outcome** | ActionOutcome (nullable) | Success details |
| **rejection** | RejectionDetails (nullable) | Failure details |
| **side_effects** | List<String> | Observable consequences |

---

## 8. Perception Schemas

### 8.1 Perception

| Field | Type | Description |
|---|---|---|
| **tick** | Integer | Current tick |
| **time_of_day** | TimeOfDay | Dawn, Morning, etc. |
| **season** | Season | Current season |
| **weather** | Weather | Current weather |
| **self_state** | SelfState | Agent's own state |
| **surroundings** | Surroundings | What's around |
| **known_routes** | List<KnownRoute> | Available paths |
| **recent_memory** | List<String> | Relevant memories |
| **available_actions** | List<String> | Valid actions |
| **notifications** | List<String> | System alerts |

### 8.2 SelfState

| Field | Type | Description |
|---|---|---|
| **id** | AgentId | Agent's ID |
| **name** | String | Agent's name |
| **age** | Integer | Current age |
| **energy** | Integer | Current energy |
| **health** | Integer | Current health |
| **hunger** | Integer | Current hunger |
| **location_name** | String | Where agent is |
| **inventory** | Map<Resource, Integer> | What agent carries |
| **carry_load** | String | "26/50" format |
| **active_goals** | List<String> | Current goals |
| **known_skills** | List<String> | "gathering (lvl 4)" format |

### 8.3 Surroundings

| Field | Type | Description |
|---|---|---|
| **location_description** | String | Narrative description |
| **visible_resources** | Map<Resource, String> | Fuzzy quantities |
| **structures_here** | List<VisibleStructure> | Buildings present |
| **agents_here** | List<VisibleAgent> | Other agents |
| **messages_here** | List<VisibleMessage> | Broadcast messages |

### 8.4 VisibleAgent

| Field | Type | Description |
|---|---|---|
| **name** | String | Agent name |
| **relationship** | String | "friendly (0.7)" format |
| **activity** | String | What they appear to be doing |

### 8.5 KnownRoute

| Field | Type | Description |
|---|---|---|
| **destination** | String | Location name |
| **cost** | String | "3 ticks" format |
| **path_type** | String | "dirt trail" format |

---

## 9. World State Schemas

### 9.1 WorldSnapshot

| Field | Type | Description |
|---|---|---|
| **tick** | Integer | Snapshot tick |
| **era** | Era | Current era |
| **season** | Season | Current season |
| **weather** | Weather | Current weather |
| **population** | PopulationStats | Population metrics |
| **economy** | EconomyStats | Economic metrics |
| **discoveries** | List<String> | All discoveries to date |
| **summary** | String | Narrative summary |

### 9.2 PopulationStats

| Field | Type | Description |
|---|---|---|
| **total_alive** | Integer | Living agents |
| **total_dead** | Integer | Deceased agents |
| **births_this_tick** | Integer | New agents |
| **deaths_this_tick** | Integer | Deaths |
| **average_age** | Float | Mean age |
| **oldest_agent** | AgentId | Longest lived |

### 9.3 EconomyStats

| Field | Type | Description |
|---|---|---|
| **total_resources** | Map<Resource, Integer> | All resources in simulation |
| **resources_in_circulation** | Map<Resource, Integer> | Resources held by agents |
| **resources_at_nodes** | Map<Resource, Integer> | Resources at locations |
| **trades_this_tick** | Integer | Trade count |
| **gini_coefficient** | Float | Wealth inequality (0–1) |

---

## 10. Dragonfly Key Patterns

Hot state stored in Dragonfly follows these key patterns:

### 10.1 World State

| Key Pattern | Type | Description |
|---|---|---|
| `world:tick` | Integer | Current tick number |
| `world:era` | String | Current era |
| `world:season` | String | Current season |
| `world:weather` | String | Current weather |
| `world:agents:alive` | Set | Living agent IDs |
| `world:agents:dead` | Set | Dead agent IDs |
| `world:locations` | Set | All location IDs |
| `world:structures` | Set | All structure IDs |

### 10.2 Agent State

| Key Pattern | Type | Description |
|---|---|---|
| `agent:{id}:vitals` | Hash | energy, health, hunger, age |
| `agent:{id}:location` | String | Current location ID |
| `agent:{id}:destination` | String | Travel destination (nullable) |
| `agent:{id}:travel_progress` | Integer | Ticks remaining |
| `agent:{id}:inventory` | Hash | Resource quantities |
| `agent:{id}:personality` | Hash | Personality vector |
| `agent:{id}:knowledge` | Set | Known concepts |
| `agent:{id}:skills` | Hash | Skill levels |
| `agent:{id}:goals` | List | Active goals |
| `agent:{id}:relationships` | Hash | AgentId â†’ score |
| `agent:{id}:memory` | List | Recent memories (JSON) |

### 10.3 Location State

| Key Pattern | Type | Description |
|---|---|---|
| `location:{id}:resources` | Hash | Resource availability |
| `location:{id}:occupants` | Set | Present agent IDs |
| `location:{id}:structures` | Set | Structure IDs here |
| `location:{id}:messages` | List | Broadcast messages |

### 10.4 Structure State

| Key Pattern | Type | Description |
|---|---|---|
| `structure:{id}:state` | Hash | Full structure state |
| `structure:{id}:occupants` | Set | Current occupants |

### 10.5 Tick Processing

| Key Pattern | Type | Description |
|---|---|---|
| `tick:{n}:actions` | List | Submitted actions queue |
| `tick:{n}:results` | Hash | AgentId â†’ result |
| `tick:{n}:events` | List | Events generated |

---

## 11. PostgreSQL Table Patterns

Persistent storage follows these table structures:

### 11.1 Core Tables

- **agents** — Immutable agent identity
- **agent_snapshots** — Periodic state snapshots
- **locations** — World geography
- **routes** — Connections between locations
- **structures** — Built structures

### 11.2 Event Tables (Partitioned)

- **events** — All simulation events, partitioned by tick range
- Partitions: events_0_10k, events_10k_20k, etc.

### 11.3 Economic Tables

- **ledger** — All resource transfers
- **trades** — Trade history

### 11.4 Analytics Tables

- **world_snapshots** — End-of-tick summaries
- **discoveries** — Knowledge milestones
- **deaths** — Death records

---

## 12. Type Generation

All types flow from Rust definitions:

### 12.1 Generation Pipeline

1. Rust structs defined in `genesis-types` crate
2. `#[derive(TS)]` macro generates TypeScript via ts-rs
3. TypeScript interfaces written to `observer/src/types/generated/`
4. Zod schemas manually maintained in `observer/src/types/schemas.ts`
5. Zod schemas import and validate against generated interfaces

### 12.2 Generation Rules

- All public types must derive `Serialize`, `Deserialize`, `TS`
- Enums generate as TypeScript string unions
- UUIDs generate as `string` type
- Timestamps generate as `string` (ISO 8601)
- Maps generate as `Record<K, V>`
- Option<T> generates as `T | null`

### 12.3 Naming Conventions

| Rust | TypeScript | Zod |
|---|---|---|
| `AgentId` | `AgentId` (type alias to string) | `AgentIdSchema` |
| `struct Agent` | `interface Agent` | `AgentSchema` |
| `enum Resource` | `type Resource = "Water" \| "Wood" \| ...` | `ResourceSchema` |

---

## 13. Simulation Run Schemas

### 13.1 SimulationRun

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **name** | String | No | Experiment name |
| **description** | String | No | Experiment description |
| **status** | SimulationStatus | No | Lifecycle state |
| **started_at_tick** | Integer | Yes | First tick of the run |
| **ended_at_tick** | Integer | Yes | Final tick of the run |
| **max_ticks** | Integer | No | Hard tick limit (0 = unlimited) |
| **config** | JSONB | No | Full config snapshot at creation time |
| **seed** | Integer (i64) | No | RNG seed for deterministic replay |
| **created_at** | Timestamp | No | Real-world creation time |
| **completed_at** | Timestamp | Yes | Real-world completion time |

### 13.2 SimulationStatus

| Status | Description |
|---|---|
| **Created** | Run configured but not yet started |
| **Running** | Tick loop is actively executing |
| **Paused** | Tick loop halted, state preserved |
| **Completed** | Run finished (time limit, extinction, era reached, or manual stop) |
| **Failed** | Run terminated due to error |

### 13.3 OperatorAction

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **run_id** | UUID v7 | No | Reference to simulation run |
| **tick** | Integer | No | Tick when action was issued |
| **action_type** | OperatorActionType | No | What the operator did |
| **parameters** | JSONB | No | Action-specific parameters |
| **created_at** | Timestamp | No | Real-world timestamp |

### 13.4 OperatorActionType

| Type | Description |
|---|---|
| **Pause** | Halt the tick loop |
| **Resume** | Resume the tick loop |
| **SetSpeed** | Change tick interval (parameters: `{ "tick_interval_ms": 5000 }`) |
| **InjectEvent** | Insert an external event into the simulation (parameters: event payload) |
| **EmergencyStop** | Immediately terminate the simulation |

---

## 14. Social Construct Schemas

### 14.1 SocialConstruct

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **name** | String | No | Name of the construct (may be agent-given or system-detected) |
| **category** | SocialConstructCategory | No | Classification |
| **description** | String | No | Description of the construct |
| **founded_at_tick** | Integer | No | Tick when first detected |
| **founded_by** | AgentId | Yes | Agent who initiated it (if identifiable) |
| **disbanded_at_tick** | Integer | Yes | Tick when dissolved (null if active) |
| **adherent_count** | Integer | No | Current number of members |
| **properties** | JSONB | No | Category-specific attributes |
| **evolution_history** | JSONB | No | Array of `{ tick, change, old_value, new_value }` entries |
| **created_at** | Timestamp | No | Real-world creation time |

### 14.2 SocialConstructCategory

| Category | Description | Example Properties |
|---|---|---|
| **Religion** | Shared belief system or mythology | deity names, rituals, sacred locations |
| **Governance** | Leadership and rule-making structure | leader_id, government_type, laws |
| **Economic** | Economic system beyond barter | currency, tax_rate, trade_agreements |
| **Family** | Kinship and partnership structure | partners, children, family_name |
| **Cultural** | Shared practices, art, traditions | customs, stories, taboos |

### 14.3 ConstructMembership

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **construct_id** | UUID v7 | No | Reference to social construct |
| **agent_id** | AgentId | No | Member agent |
| **joined_at_tick** | Integer | No | When the agent joined |
| **left_at_tick** | Integer | Yes | When the agent left (null if active) |
| **role** | String | No | Role within the construct (e.g., "leader", "member", "priest") |
| **created_at** | Timestamp | No | Real-world creation time |

---

## 15. Deception & Reputation Schemas

### 15.1 DeceptionRecord

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **tick** | Integer | No | When the deception occurred |
| **deceiver_id** | AgentId | No | Agent who lied |
| **target_id** | AgentId | No | Agent who was lied to |
| **deception_type** | String | No | Category of deception (e.g., "resource_lie", "location_lie", "relationship_lie") |
| **claimed_info** | JSONB | No | What the deceiver said |
| **actual_truth** | JSONB | No | What was actually true |
| **discovered** | Boolean | No | Whether the deception has been uncovered (default: false) |
| **discovered_at_tick** | Integer | Yes | Tick when discovered |
| **discovered_by** | AgentId | Yes | Agent who uncovered the deception |
| **created_at** | Timestamp | No | Real-world creation time |

### 15.2 ReputationEvent

| Field | Type | Nullable | Description |
|---|---|---|---|
| **id** | UUID v7 | No | Unique identifier |
| **tick** | Integer | No | When the action was observed |
| **subject_id** | AgentId | No | Agent whose reputation changed |
| **observer_id** | AgentId | No | Agent who observed the action |
| **action_type** | String | No | What action was observed (e.g., "generous_trade", "theft", "teaching") |
| **reputation_delta** | Numeric | No | Signed reputation change (positive = good, negative = bad) |
| **context** | JSONB | No | Action-specific details |
| **created_at** | Timestamp | No | Real-world creation time |

---

## 16. New Action Parameters

### 16.1 Steal

| Field | Type |
|---|---|
| target_agent | AgentId |
| resource | Resource |

### 16.2 Attack

| Field | Type |
|---|---|
| target_agent | AgentId |

### 16.3 Propose

| Field | Type |
|---|---|
| proposal_type | String (alliance, treaty, trade_agreement, law) |
| target_agents | List<AgentId> |
| terms | JSONB |

### 16.4 Vote

| Field | Type |
|---|---|
| proposal_id | UUID |
| vote | Boolean (approve/reject) |

### 16.5 Marry

| Field | Type |
|---|---|
| partner_agent | AgentId |

### 16.6 Divorce

| Field | Type |
|---|---|
| partner_agent | AgentId |

### 16.7 Conspire

| Field | Type |
|---|---|
| target_agents | List<AgentId> |
| message | String (max 500 chars) |

### 16.8 Pray

| Field | Type |
|---|---|
| construct_id | UUID (nullable, may pray without formal religion) |
| prayer | String (max 200 chars) |

---

## 17. New Event Details

### 17.1 TheftOccurred / TheftFailed

| Field | Type |
|---|---|
| thief_id | AgentId |
| victim_id | AgentId |
| resource | Resource |
| quantity | Integer |
| detected | Boolean |

### 17.2 CombatInitiated / CombatResolved

| Field | Type |
|---|---|
| attacker_id | AgentId |
| defender_id | AgentId |
| cause | String |
| outcome | String (attacker_won, defender_won, draw, fled) |
| attacker_damage | Integer |
| defender_damage | Integer |

### 17.3 DeceptionCommitted / DeceptionDiscovered

| Field | Type |
|---|---|
| deception_record_id | UUID |
| deceiver_id | AgentId |
| target_id | AgentId |
| deception_type | String |

### 17.4 AllianceFormed / AllianceBroken

| Field | Type |
|---|---|
| alliance_members | List<AgentId> |
| alliance_name | String |
| reason | String |

### 17.5 WarDeclared / TreatyNegotiated

| Field | Type |
|---|---|
| party_a | List<AgentId> |
| party_b | List<AgentId> |
| terms | JSONB |

### 17.6 SocialConstructFormed / SocialConstructDisbanded

| Field | Type |
|---|---|
| construct_id | UUID |
| construct_name | String |
| category | SocialConstructCategory |
| adherent_count | Integer |

### 17.7 ReputationChanged

| Field | Type |
|---|---|
| subject_id | AgentId |
| observer_id | AgentId |
| delta | Numeric |
| cause | String |

### 17.8 OperatorAction

| Field | Type |
|---|---|
| operator_action_id | UUID |
| action_type | OperatorActionType |
| parameters | JSONB |

### 17.9 SimulationPaused / SimulationResumed / SimulationEnded

| Field | Type |
|---|---|
| run_id | UUID |
| reason | String |
| final_tick | Integer (SimulationEnded only) |
| final_population | Integer (SimulationEnded only) |

---

*This document is the canonical type reference. When in doubt, this is the source of truth.*
