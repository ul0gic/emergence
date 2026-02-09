# Agent System — Technical Specification

> **Project:** Emergence
> **Component:** Agent Runtime
> **Status:** Design
> **Date:** February 2026

---

## 1. Overview

Agents are the inhabitants of the Genesis simulation. Each agent is an autonomous entity with persistent identity, memory, knowledge, personality, and goals. Agents make decisions through LLM inference based on their perception of the world.

**The Agent System is NOT the World Engine.** The World Engine enforces physics. Agents make choices within those physics. The Agent System handles perception assembly, LLM inference, action parsing, and memory management.

---

## 2. Agent Identity

Every agent has a permanent identity established at creation (birth or simulation seed).

### 2.1 Core Identity Attributes

| Attribute | Type | Mutability | Description |
|---|---|---|---|
| **id** | UUID v7 | Immutable | Unique identifier, time-ordered |
| **name** | String | Immutable | Display name, unique within simulation |
| **born_at_tick** | Integer | Immutable | Tick when agent entered simulation |
| **parent_a** | UUID (nullable) | Immutable | First parent if reproduced, null if seed agent |
| **parent_b** | UUID (nullable) | Immutable | Second parent if reproduced, null if seed agent |
| **personality** | Personality | Immutable | Core personality vector (see 2.2) |
| **generation** | Integer | Immutable | 0 for seed agents, increments for children |

### 2.2 Personality Model

Personality is a fixed vector of traits assigned at birth. It influences decision-making but does not change over the agent's lifetime.

| Trait | Range | Description |
|---|---|---|
| **curiosity** | 0.0–1.0 | Likelihood to explore, try new things, learn from observation |
| **cooperation** | 0.0–1.0 | Preference for collaboration vs. solo action |
| **aggression** | 0.0–1.0 | Tendency toward conflict, competition, dominance |
| **risk_tolerance** | 0.0–1.0 | Willingness to take uncertain actions |
| **industriousness** | 0.0–1.0 | Preference for productive work vs. rest/leisure |
| **sociability** | 0.0–1.0 | Desire for interaction vs. solitude |
| **honesty** | 0.0–1.0 | Tendency toward truthful communication |
| **loyalty** | 0.0–1.0 | Commitment to relationships and groups |

**Inheritance:** Child agents receive a blended personality from parents with random mutation (Â±0.1 per trait, clamped to valid range).

**Seed agents:** Personality is randomly generated with configurable distribution (uniform, normal around 0.5, or operator-specified).

---

## 3. Agent State

State is the mutable aspect of an agent — everything that changes as the agent lives.

### 3.1 Vital Statistics

| Stat | Range | Default | Tick Behavior |
|---|---|---|---|
| **energy** | 0–100 | 80 | Decreases with actions, increases with rest/eating |
| **health** | 0–100 | 100 | Decreases from starvation/injury, recovers when conditions met |
| **hunger** | 0–100 | 0 | Increases each tick, resets when eating |
| **age** | 0–lifespan | 0 | Increases by 1 each tick |

### 3.2 Inventory (Wallet)

Agents carry resources in their inventory. Each resource type maps to a quantity.

| Property | Description |
|---|---|
| **contents** | Map of resource type to quantity (unsigned integer) |
| **carry_capacity** | Maximum total units agent can carry (default: 50) |
| **current_load** | Sum of all quantities in contents |

Agents cannot gather or receive resources if it would exceed carry capacity.

### 3.3 Location

| Property | Description |
|---|---|
| **current_location** | UUID of location where agent currently is |
| **destination** | UUID of location agent is traveling to (nullable) |
| **travel_progress** | Ticks remaining until arrival (0 if not traveling) |

Agents in transit cannot perform most actions until they arrive.

### 3.4 Social Graph

Agents maintain relationships with other agents they have encountered.

| Property | Description |
|---|---|
| **relationships** | Map of agent UUID to relationship score (-1.0 to 1.0) |
| **interaction_count** | Map of agent UUID to number of interactions |
| **last_interaction** | Map of agent UUID to tick of last interaction |

Relationship scores evolve based on interactions:
- Positive trade: +0.1
- Failed/rejected trade: -0.05
- Teaching received: +0.15
- Positive communication: +0.05
- Conflict: -0.2 to -0.5 depending on severity

### 3.5 Knowledge Base

What the agent knows. Determines available actions and effectiveness.

| Property | Description |
|---|---|
| **known_concepts** | Set of knowledge identifiers the agent has acquired |
| **discovery_tick** | Map of concept to tick when learned |
| **discovery_method** | Map of concept to how it was learned (seed, discovery, taught, read) |

Knowledge is never lost unless explicitly modeled (e.g., aging memory loss, which is not in Phase 0).

### 3.6 Skills

Proficiency levels that improve with use.

| Property | Description |
|---|---|
| **skill_levels** | Map of skill name to level (integer, starting at 1) |
| **skill_xp** | Map of skill name to experience points toward next level |

Skills affect action outcomes:
- Gathering yield = base_yield + (skill_level * 0.5)
- Building speed = base_time / (1 + skill_level * 0.1)
- Teaching success = base_rate + (skill_level * 0.05)

### 3.7 Goals

Agent-generated objectives that influence decision-making.

| Property | Description |
|---|---|
| **active_goals** | Ordered list of current goals (strings, max 5) |
| **completed_goals** | List of achieved goals with completion tick |

Goals are generated by the agent during reflection and included in perception to provide continuity.

---

## 4. Memory System

Agents have limited memory. They cannot recall every event. Memory is managed through summarization and relevance filtering.

### 4.1 Memory Tiers

| Tier | Retention | Content |
|---|---|---|
| **Immediate** | Last 5 ticks | Full detail of all perceived events |
| **Short-term** | Last 50 ticks | Summarized events, key interactions |
| **Long-term** | Lifetime | Major milestones, relationship formation, discoveries, deaths |

### 4.2 Memory Entry Structure

| Field | Description |
|---|---|
| **tick** | When the event occurred |
| **type** | Category (action, observation, communication, discovery, social) |
| **summary** | Human-readable description |
| **entities** | List of agent/location/resource UUIDs involved |
| **emotional_weight** | Significance score (0.0–1.0) — higher = more likely retained |

### 4.3 Memory Compression

At the end of each tick, during the Reflection phase:
1. Immediate memories older than 5 ticks are evaluated
2. High emotional weight events (>0.7) promote to long-term
3. Medium weight events (0.3–0.7) summarize into short-term
4. Low weight events (<0.3) are discarded
5. Short-term memories older than 50 ticks are re-evaluated for long-term promotion or discard

### 4.4 Memory in Perception

The perception payload includes:
- All immediate memories (last 5 ticks, full detail)
- Relevant short-term memories (filtered by current location, nearby agents, active goals)
- All long-term memories (always included, typically small set)

Maximum memory tokens in perception: configurable, default 2000 tokens.

---

## 5. Perception

Perception is the world state as seen by a specific agent. It is assembled by the World Engine and delivered to the Agent Runner at the start of each tick.

### 5.1 Perception Principles

1. **Fog of war** — Agents only perceive their current location and known routes
2. **No omniscience** — Agents cannot see other agents' inventories, stats, or thoughts
3. **Fuzzy quantities** — Resource amounts shown as ranges, not exact numbers
4. **Relationship context** — Other agents shown with relationship status
5. **Temporal context** — Current tick, season, weather, time of day included

### 5.2 Perception Payload Structure

| Section | Contents |
|---|---|
| **meta** | Tick number, time of day, season, weather |
| **self** | Agent's own stats, inventory, load, goals, skills |
| **location** | Current location name, description, structures, resource availability (fuzzy) |
| **agents_present** | Other agents at same location with name, relationship, visible activity |
| **routes** | Known routes from current location with destination, cost, path type |
| **messages** | Recent broadcast messages at this location |
| **memory** | Filtered memories as described in section 4.4 |
| **notifications** | System alerts (approaching winter, low health, shelter damage) |
| **available_actions** | Actions the agent can currently perform given their knowledge and state |

### 5.3 Fuzzy Resource Representation

Exact quantities are hidden. Agents see:

| Actual Quantity | Display |
|---|---|
| 0 | "none" |
| 1–5 | "scarce" |
| 6–15 | "limited" |
| 16–30 | "moderate" |
| 31–60 | "abundant" |
| 61+ | "plentiful" |

This prevents agents from gaming with exact calculations.

---

## 6. Decision Making

The Agent Runner receives perception and produces an action through LLM inference.

### 6.1 Decision Loop

1. **Receive perception** — World Engine publishes perception payload
2. **Assemble prompt** — Combine perception with agent identity, personality, and instructions
3. **LLM inference** — Call language model with assembled prompt
4. **Parse response** — Extract structured action from model output
5. **Validate format** — Ensure action matches expected schema
6. **Submit action** — Send to World Engine action queue
7. **Handle timeout** — If deadline exceeded, submit NO_ACTION

### 6.2 Prompt Structure

The prompt to the LLM follows this structure:

| Section | Purpose |
|---|---|
| **System context** | You are an agent in a simulation, your goal is survival and flourishing |
| **Identity** | Name, age, personality traits as natural language |
| **Current state** | Vitals, inventory, location, goals |
| **Perception** | What you see, who is here, available resources |
| **Memory** | Recent events and relevant history |
| **Available actions** | What you can do right now |
| **Response format** | Instructions for structured output |

### 6.3 Response Format

The agent must respond with a structured action containing:

| Field | Required | Description |
|---|---|---|
| **action** | Yes | Action type identifier |
| **parameters** | Depends | Action-specific parameters |
| **reasoning** | No | Agent's internal reasoning (logged but not used) |
| **goal_update** | No | New goals or goal completion signals |

### 6.4 Action Timeout

Agents have a deadline to respond (default: 8 seconds). If exceeded:
- Agent forfeits the tick
- NO_ACTION recorded in event log
- Vitals still update (hunger increases, etc.)
- Agent can act next tick normally

---

## 7. Actions

Actions are what agents can do. The full catalog is defined in the World Engine specification. This section covers the agent-side contract.

### 7.1 Action Request Structure

| Field | Type | Description |
|---|---|---|
| **agent_id** | UUID | Who is acting |
| **tick** | Integer | Current tick (for validation) |
| **action_type** | Enum | The action being taken |
| **parameters** | Object | Action-specific parameters |
| **submitted_at** | Timestamp | When action was submitted (for ordering) |

### 7.2 Action Categories

| Category | Actions | Notes |
|---|---|---|
| **Survival** | gather, eat, drink, rest, move | Always available based on context |
| **Construction** | build, repair, demolish, improve_route | Require knowledge and materials |
| **Social** | communicate, broadcast, trade_offer, trade_accept, trade_reject, form_group, teach | Require other agents |
| **Advanced** | farm_plant, farm_harvest, craft, mine, smelt, write, read, claim, legislate, enforce, reproduce | Require specific knowledge/structures |

### 7.3 Action Parameters by Type

| Action | Required Parameters |
|---|---|
| **gather** | resource (string) |
| **eat** | food_type (string, from inventory) |
| **move** | destination (location UUID or name) |
| **build** | structure_type (string) |
| **repair** | structure_id (UUID) |
| **communicate** | target_agent (UUID or name), message (string) |
| **broadcast** | message (string) |
| **trade_offer** | target_agent, offer (resource map), request (resource map) |
| **trade_accept** | trade_id (UUID) |
| **trade_reject** | trade_id (UUID) |
| **teach** | target_agent, knowledge (string) |
| **reproduce** | partner_agent (UUID or name) |

### 7.4 Action Results

After resolution, agents receive results:

| Field | Description |
|---|---|
| **success** | Boolean — did the action succeed |
| **outcome** | What happened (resources gained, trade completed, etc.) |
| **rejection_reason** | If failed, why (insufficient energy, wrong location, etc.) |
| **side_effects** | Observable consequences (other agent noticed you, weather changed) |

---

## 8. Reflection

After receiving action results, agents update their internal state. This happens asynchronously and does not block the next tick.

### 8.1 Reflection Tasks

1. **Memory formation** — Convert action result and observations into memory entries
2. **Goal evaluation** — Mark goals as completed or update progress
3. **Goal generation** — Create new goals based on current situation
4. **Relationship update** — Adjust social graph based on interactions

### 8.2 Goal Generation Heuristics

Agents generate goals based on:
- **Survival needs** — Low energy â†’ "rest", high hunger â†’ "find food"
- **Resource gaps** — No shelter â†’ "build shelter", low inventory â†’ "gather supplies"
- **Social state** — No relationships â†’ "meet other agents", high relationship â†’ "trade with X"
- **Curiosity** — Unknown locations nearby â†’ "explore", observed unknown action â†’ "learn that"
- **Personality** — High industriousness â†’ more work goals, high sociability â†’ more social goals

### 8.3 Reflection Output

| Field | Description |
|---|---|
| **new_memories** | Memory entries to store |
| **updated_goals** | Replacement goal list |
| **internal_state** | Any agent-managed state updates |

---

## 9. Agent Lifecycle

### 9.1 Birth (Seed Agents)

At simulation start, seed agents are created with:
- Generated UUID and name
- Random personality (or operator-specified)
- Seed knowledge based on configured level
- Starting inventory based on configuration
- Placed at designated starting locations
- Age 0, full health and energy, zero hunger

### 9.2 Birth (Reproduction)

When two agents reproduce:
- New UUID generated, name can be parent-chosen or generated
- Personality blended from parents with mutation
- Knowledge inherited: intersection of parent knowledge, minus advanced concepts
- Starting inventory: zero (parents may gift resources)
- Location: same as parents
- Age 0, full health and energy, zero hunger
- Maturity period: reduced action capacity for N ticks (configurable)

### 9.3 Aging

- Age increments by 1 each tick
- At 80% of lifespan: energy cap begins declining
- At 90% of lifespan: movement cost increases
- At 100% of lifespan: agent dies of old age

### 9.4 Death

Agents die when:
- Health reaches 0
- Age exceeds lifespan

On death:
- Agent removed from active simulation
- Final state snapshot recorded
- Inventory dropped at current location
- Structures become unowned
- Social connections notified in next perception
- DEATH event emitted

### 9.5 Death Notification

Agents who had relationship with deceased receive notification:
- Included in next tick's perception
- Memory entry created automatically
- Relationship entry preserved but marked as deceased

---

## 10. Agent Runner Architecture

The Agent Runner is a separate Rust binary (`emergence-runner`) that manages agent decision-making. It runs alongside the World Engine but is a distinct process, communicating exclusively via NATS. Both are built from the same Cargo workspace and share types via `emergence-types`.

### 10.1 Responsibilities

| Responsibility | Description |
|---|---|
| **Perception intake** | Subscribe to perception payloads from World Engine via NATS |
| **Prompt assembly** | Build LLM prompts for each agent using `minijinja` templates |
| **LLM orchestration** | Manage inference calls via `reqwest` (parallel, rate-limited, timeout-aware) |
| **Response parsing** | Extract structured actions from LLM output with compile-time type safety |
| **Action submission** | Send validated actions to World Engine via NATS |
| **Reflection execution** | Run reflection after results received |
| **Memory persistence** | Store agent memory state in Dragonfly |

### 10.2 Why Rust (not Python)

All LLM backends (DeepSeek, OpenAI, Claude, Ollama) expose OpenAI-compatible REST APIs. The agent runner is fundamentally "receive JSON → build prompt string → call HTTP endpoint → parse JSON response → send JSON." This does not require Python's ML ecosystem.

**Benefits of all-Rust:**
- Single `cargo build` compiles the entire simulation (engine + runner)
- Perception and action types from `emergence-types` are compile-time checked end-to-end
- One Docker image, one dependency tree, one language to maintain
- Prompt templates loaded from files — iteration speed is template editing, not recompilation

### 10.3 Scaling Model

| Agents | Runner Instances | Notes |
|---|---|---|
| 1–10 | 1 | Single process, sequential or parallel calls |
| 11–50 | 1–2 | Parallel calls with connection pooling |
| 51–200 | 2–5 | Multiple runners, agents partitioned |
| 200+ | 5+ | Horizontal scaling, load balancer |

### 10.4 LLM Backend Configuration

All backends use the OpenAI-compatible chat completions API. Swapping backends is a URL + API key change.

| Backend | Endpoint | Use Case |
|---|---|---|
| **DeepSeek API** | `https://api.deepseek.com/v1/chat/completions` | Cost-efficient routine decisions |
| **OpenAI API** | `https://api.openai.com/v1/chat/completions` | Cheap fast models for routine decisions |
| **Claude API** | `https://api.anthropic.com/v1/messages` | High-quality decisions, complex social situations |
| **Ollama (local)** | `http://localhost:11434/v1/chat/completions` | Offline operation, maximum cost control |

Configuration allows:
- Default backend for routine decisions
- Escalation backend for complex situations (discoveries, conflicts, major social events)
- Fallback backend if primary fails

### 10.5 Error Handling

| Error | Handling |
|---|---|
| LLM timeout | Submit NO_ACTION, log warning |
| LLM rate limit | Retry with exponential backoff, submit NO_ACTION if deadline exceeded |
| Parse failure | Attempt recovery parse, submit NO_ACTION if unrecoverable |
| Invalid action | Submit NO_ACTION, log the attempted action for debugging |
| Network failure | Retry once, submit NO_ACTION if still failing |

---

## 11. Communication Protocol

### 11.1 World Engine â†’ Agent Runner

| Message | Content | Delivery |
|---|---|---|
| **PERCEPTION** | Full perception payload for one agent | Pub/sub, one message per agent per tick |
| **ACTION_RESULT** | Outcome of submitted action | Pub/sub, one message per agent per tick |
| **TICK_START** | Tick number, deadline timestamp | Pub/sub, broadcast |
| **TICK_END** | Tick number, summary | Pub/sub, broadcast |

### 11.2 Agent Runner â†’ World Engine

| Message | Content | Delivery |
|---|---|---|
| **ACTION** | Action request for one agent | Queue, ordered by submission time |
| **NO_ACTION** | Explicit forfeit for one agent | Queue |

### 11.3 Timing Contract

1. World Engine emits TICK_START with deadline
2. World Engine emits PERCEPTION for each agent (parallel)
3. Agent Runners must submit ACTION or NO_ACTION before deadline
4. World Engine processes actions after deadline (no late submissions accepted)
5. World Engine emits ACTION_RESULT for each agent
6. World Engine emits TICK_END
7. Agent Runners perform reflection (async, no deadline)

---

## 12. Configuration

Agent system parameters configurable at simulation start:

| Parameter | Default | Description |
|---|---|---|
| **decision_timeout_ms** | 8000 | Max time for agent to respond |
| **max_memory_tokens** | 2000 | Memory included in perception |
| **immediate_memory_ticks** | 5 | Full-detail memory retention |
| **short_term_memory_ticks** | 50 | Summarized memory retention |
| **max_active_goals** | 5 | Goals per agent |
| **personality_mutation_range** | 0.1 | Inheritance variation |
| **maturity_ticks** | 200 | Child development period |
| **llm_default_backend** | "deepseek" | Primary LLM provider |
| **llm_escalation_backend** | "claude" | High-stakes LLM provider |
| **parallel_inference** | true | Concurrent LLM calls |
| **max_concurrent_calls** | 20 | Rate limiting |

---

*Agents are the soul of the simulation. This document defines how they perceive, think, and act within the reality the World Engine provides.*
