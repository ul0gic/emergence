# Emergence — Build Plan

> **CRITICAL INSTRUCTIONS FOR ENGINEERS**
>
> ## Project Structure
> All project documentation lives in the `.project/` directory at the repository root:
> ```
> .project/
> ├── prd.md             # Product Requirements Document
> ├── tech-stack.md      # Technology choices and rationale
> ├── build-plan.md      # This file — task tracking
> ├── changelog.md       # Version history and updates
> ├── data-schemas.md    # Canonical data type definitions
> ├── agent-system.md    # Agent runtime specification
> └── world-engine.md    # World Engine technical design
> ```
>
> ## Build Discipline
> 1. **Keep this document up to date** — Mark tasks as completed immediately after finishing them
> 2. **Build after every task** — Run the build command after completing each task
> 3. **Zero tolerance for warnings/errors** — Fix any warnings or errors before moving to the next task
> 4. **Update changelog.md** — Log significant changes, fixes, and milestones
>
> ```bash
> # Build commands (run after each task)
> cargo build                          # Rust workspace
> cargo clippy -- -D warnings          # Lint check
> cargo test                           # Run tests
> ```
>
> If warnings or errors appear, fix them immediately. Do not proceed until the build is clean.

---

## Status Legend

| Icon | Status | Description |
|------|--------|-------------|
| ⬜ | Not Started | Task has not begun |
| 🔄 | In Progress | Currently being worked on |
| ✅ | Completed | Task finished |
| ⛔ | Blocked | Cannot proceed due to external dependency |
| ⚠️ | Has Blockers | Waiting on another task |
| 🔍 | In Review | Pending review/approval |
| 🚫 | Skipped | Intentionally not doing |
| ⏸️ | Deferred | Postponed to later phase |

---

## Project Progress Summary

```
Phase 0: Project Setup          [████████████████████] 100%  ✅
Phase 1: Foundation              [████████████████████] 100%  ✅
Phase 2: Primitive World         [████████████████████] 100%  ✅
Phase 3: Society                 [████████████████████] 100%  ✅
Phase 4: Complexity              [████████████████████] 100%  ✅
Phase 5: Scale & Research        [░░░░░░░░░░░░░░░░░░░░]   0%  ⬜
Phase 6: Open World & Emergence  [███░░░░░░░░░░░░░░░░░]  16%  🔄
─────────────────────────────────────────────────────────────
Overall Progress                 [██████████████░░░░░░]  73%
```

| Phase | Tasks | Completed | Blocked | Deferred | Progress |
|-------|-------|-----------|---------|----------|----------|
| Phase 0: Project Setup | 18 | 18 | 0 | 0 | 100% |
| Phase 1: Foundation | 30 | 30 | 0 | 0 | 100% |
| Phase 2: Primitive World | 29 | 29 | 0 | 0 | 100% |
| Phase 3: Society | 24 | 24 | 0 | 0 | 100% |
| Phase 4: Complexity | 20 | 20 | 0 | 0 | 100% |
| Phase 5: Scale & Research | 16 | 0 | 0 | 0 | 0% |
| Phase 6: Open World & Emergence | 37 | 6 | 0 | 0 | 16% |
| **Total** | **174** | **127** | **0** | **0** | **73%** |

---

## Phase 0: Project Setup

> Scaffolding, tooling, and infrastructure. Nothing runs yet but everything compiles.

### 0.1 Repository & Workspace

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 0.1.1 | Initialize git repository with `.gitignore` (Rust, Python, Node, Docker, `.env`) |
| ✅ | 0.1.2 | Create Rust workspace `Cargo.toml` with all crate members, workspace dependencies, and lint config (see world-engine.md §14.3) |
| ✅ | 0.1.3 | Scaffold all 8 crates: `emergence-types`, `emergence-core`, `emergence-world`, `emergence-agents`, `emergence-ledger`, `emergence-events`, `emergence-db` (lib crates with `lib.rs` stubs), `emergence-runner` (binary crate with `main.rs` stub) |
| ✅ | 0.1.4 | Create `src/main.rs` entry point stub |
| ✅ | 0.1.5 | Verify workspace compiles clean with full lint config (`cargo build && cargo clippy`) |
| ✅ | 0.1.6 | **BUILD CHECK** — Clean build, zero warnings, all lints passing |

### 0.2 Infrastructure Setup

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 0.2.1 | Create `docker-compose.yml` with Dragonfly, PostgreSQL, and NATS services |
| ✅ | 0.2.2 | Create `.env.example` with all required environment variables |
| ✅ | 0.2.3 | Create `emergence-config.yaml` with default simulation parameters (see world-engine.md §13) |
| ✅ | 0.2.4 | Verify `docker compose up` starts all services and they're reachable |
| ✅ | 0.2.5 | **BUILD CHECK** — All infrastructure services running and connectable |

### 0.3 Database & Migrations

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 0.3.1 | Set up `sqlx` migrations in `emergence-db` crate |
| ✅ | 0.3.2 | Create initial migration: `agents`, `locations`, `routes`, `structures` tables (see world-engine.md §10.2) |
| ✅ | 0.3.3 | Create migration: `ledger` table with indexes (see data-schemas.md §6) |
| ✅ | 0.3.4 | Create migration: `events` table with tick-range partitioning (see world-engine.md §10.2) |
| ✅ | 0.3.5 | Create migration: `discoveries`, `agent_snapshots`, `world_snapshots` tables |
| ✅ | 0.3.6 | Verify migrations run clean against Docker PostgreSQL |
| ✅ | 0.3.7 | **BUILD CHECK** — `cargo sqlx prepare` succeeds, all queries compile |

---

## Phase 1: Foundation

> World Engine core: clock, types, state management, ledger, single agent proof-of-concept. The physics engine works.

### 1.1 Shared Types (`emergence-types`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 1.1.1 | Define type-safe ID wrappers: `AgentId`, `LocationId`, `StructureId`, `RouteId`, `EventId`, `TradeId`, `GroupId`, `LedgerEntryId` (UUID v7). Note: PostgreSQL 18 generates UUIDs via native `DEFAULT uuidv7()` — Rust wrappers are for type safety and deserialization, not generation. |
| ✅ | 1.1.2 | Define resource enum with all tiers (see data-schemas.md §3.1) |
| ✅ | 1.1.3 | Define structure types enum (see data-schemas.md §3.2) |
| ✅ | 1.1.4 | Define action types enum (see data-schemas.md §3.3) |
| ✅ | 1.1.5 | Define event types enum (see data-schemas.md §3.4) |
| ✅ | 1.1.6 | Define rejection reasons enum (see data-schemas.md §3.5) |
| ✅ | 1.1.7 | Define environment enums: `Season`, `Weather`, `PathType`, `TimeOfDay`, `Era` (see data-schemas.md §3.6–3.10) |
| ✅ | 1.1.8 | Define `Personality` struct with 8 trait fields (see data-schemas.md §4.2) |
| ✅ | 1.1.9 | Define `ActionRequest`, `ActionResult`, `ActionOutcome` structs (see data-schemas.md §7) |
| ✅ | 1.1.10 | Define `Perception`, `SelfState`, `Surroundings`, `VisibleAgent`, `KnownRoute` structs (see data-schemas.md §8) |
| ✅ | 1.1.11 | Add `#[derive(TS)]` to all public types for TypeScript generation |
| ✅ | 1.1.12 | **BUILD CHECK** — All types compile, `cargo test` generates TypeScript bindings |

### 1.2 World Clock & Environment (`emergence-core`, `emergence-world`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 1.2.1 | Implement world clock: tick counter, era tracker (see world-engine.md §2) |
| ✅ | 1.2.2 | Implement time-of-day cycle (dawn/morning/afternoon/dusk/night) |
| ✅ | 1.2.3 | Implement season cycle (90 ticks per season, configurable) |
| ✅ | 1.2.4 | Implement weather system (random per tick, weighted by season) |
| ✅ | 1.2.5 | Implement configuration loading from `emergence-config.yaml` |
| ✅ | 1.2.6 | **BUILD CHECK** — Clock advances, seasons rotate, weather generates |

### 1.3 World Geography (`emergence-world`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 1.3.1 | Implement `Location` struct with resources, capacity, occupants (see world-engine.md §3.2) |
| ✅ | 1.3.2 | Implement `ResourceNode` with regeneration logic (see data-schemas.md §4.5) |
| ✅ | 1.3.3 | Implement `Route` struct with travel cost, path types, ACLs (see world-engine.md §3.3) |
| ✅ | 1.3.4 | Implement world graph: locations as nodes, routes as weighted edges |
| ✅ | 1.3.5 | Create starting world map: 8–12 locations across 3 regions (see world-engine.md §3.4) |
| ✅ | 1.3.6 | **BUILD CHECK** — World graph loads, resources regenerate per tick |

### 1.4 Central Ledger (`emergence-ledger`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 1.4.1 | Implement `LedgerEntry` struct with all entry types (see data-schemas.md §6) |
| ✅ | 1.4.2 | Implement double-entry bookkeeping: every transfer has debit + credit |
| ✅ | 1.4.3 | Implement conservation law check: total resources in = total resources out per tick |
| ✅ | 1.4.4 | Implement `LEDGER_ANOMALY` alert when conservation law is violated |
| ✅ | 1.4.5 | Write ledger unit tests: transfers, regeneration, consumption, anomaly detection |
| ✅ | 1.4.6 | **BUILD CHECK** — Ledger balances on every test scenario |

### 1.5 Data Layer (`emergence-db`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 1.5.1 | Implement Dragonfly connection and key operations (world state, agent state, location state) (see data-schemas.md §10) |
| ✅ | 1.5.2 | Implement PostgreSQL connection with `sqlx` connection pool |
| ✅ | 1.5.3 | Implement event store: batch insert events with tick partitioning |
| ✅ | 1.5.4 | Implement ledger persistence: batch insert ledger entries |
| ✅ | 1.5.5 | Implement world snapshot persistence |
| ✅ | 1.5.6 | **BUILD CHECK** — Read/write to both Dragonfly and PostgreSQL verified |

---

## Phase 2: Primitive World

> Multiple agents running, survival mechanics, basic interaction, basic observer. The simulation lives.

### 2.1 Agent State & Vitals (`emergence-agents`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.1.1 | Implement `Agent` struct with identity, personality, generation (see agent-system.md §2) |
| ✅ | 2.1.2 | Implement `AgentState` with vitals: energy, health, hunger, age (see agent-system.md §3.1) |
| ✅ | 2.1.3 | Implement inventory/wallet with carry capacity (see agent-system.md §3.2) |
| ✅ | 2.1.4 | Implement vital mechanics per tick: hunger increase, energy depletion, health recovery, aging (see world-engine.md §6.2) |
| ✅ | 2.1.5 | Implement death conditions: health = 0, age > lifespan (see agent-system.md §9.4) |
| ✅ | 2.1.6 | Implement death consequences: inventory drop, structure orphan, social notification |
| ✅ | 2.1.7 | **BUILD CHECK** — Agent vitals tick correctly, death triggers properly |

### 2.2 Tick Cycle (`emergence-core`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.2.1 | Implement Phase 1 (World Wake): advance clock, apply environment, regenerate resources, decay structures, apply hunger, age agents, kill dead agents (see world-engine.md §2.1) |
| ✅ | 2.2.2 | Implement Phase 2 (Perception): assemble perception payload per agent from world state (see world-engine.md §11) |
| ✅ | 2.2.3 | Implement fuzzy resource representation in perception (see agent-system.md §5.3) |
| ✅ | 2.2.4 | Implement Phase 3 (Decision): publish perception to NATS, await action responses with timeout |
| ✅ | 2.2.5 | Implement Phase 4 (Resolution): validate actions, resolve conflicts, execute valid actions, reject invalid (see world-engine.md §7.2) |
| ✅ | 2.2.6 | Implement Phase 5 (Persist): flush state to PostgreSQL, publish tick summary |
| ✅ | 2.2.7 | Implement tick timing: configurable interval, deadline enforcement |
| ✅ | 2.2.8 | **BUILD CHECK** — Full tick cycle runs end-to-end with test agents |

### 2.3 Action System (`emergence-agents`, `emergence-core`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.3.1 | Implement survival actions: `gather`, `eat`, `drink`, `rest`, `move` (see world-engine.md §7.1) |
| ✅ | 2.3.2 | Implement action validation pipeline: syntax → vitals → location → resources → world state → skill → conflict (see world-engine.md §7.2) |
| ✅ | 2.3.3 | Implement conflict resolution: first-come-first-served, splitting, bidding, rejection (see world-engine.md §2.3) |
| ✅ | 2.3.4 | Implement action energy costs and resource effects |
| ✅ | 2.3.5 | Implement `move` action with multi-tick travel and route cost calculation |
| ✅ | 2.3.6 | **BUILD CHECK** — All survival actions validate and execute correctly |

### 2.4 Agent Runner (`emergence-runner`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.4.1 | Scaffold `emergence-runner` binary crate in workspace with `main.rs`, add `reqwest` and `minijinja` dependencies |
| ✅ | 2.4.2 | Implement NATS subscription: receive perception, submit actions (using `async-nats` + `emergence-types`) |
| ✅ | 2.4.3 | Create prompt templates (`templates/` dir) and implement `minijinja` template loading: system context + identity + perception + memory + available actions (see agent-system.md §6.2) |
| ✅ | 2.4.4 | Implement LLM backend abstraction: OpenAI-compatible HTTP client via `reqwest` with configurable endpoint URL + API key for OpenAI and Anthropic (two-tier fallback) |
| ✅ | 2.4.5 | Implement response parsing: extract structured action from LLM JSON output with `serde` deserialization into `ActionRequest` |
| ✅ | 2.4.6 | Implement timeout handling: forfeit tick with `NO_ACTION` if deadline exceeded |
| ✅ | 2.4.7 | Implement fallback chain: default backend → escalation backend → NO_ACTION |
| ✅ | 2.4.8 | **BUILD CHECK** — Single agent receives perception, calls LLM, submits valid action |

### 2.5 Basic Social Actions

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.5.1 | Implement `communicate` action: direct message between co-located agents |
| ✅ | 2.5.2 | Implement `broadcast` action: post message visible to all at location |
| ✅ | 2.5.3 | Implement message board: messages stored per location in Dragonfly, included in perception |
| ✅ | 2.5.4 | **BUILD CHECK** — Agents can send and receive messages |

### 2.6 Basic Observer

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 2.6.1 | Implement Axum WebSocket endpoint for tick summary streaming |
| ✅ | 2.6.2 | Implement REST endpoints: list agents, get agent state, list locations, get events |
| ✅ | 2.6.3 | Create minimal Observer backend: Axum HTTP server with REST API + WebSocket streaming |
| ✅ | 2.6.4 | **BUILD CHECK** — Observer backend serves API, WebSocket streams tick data |

---

## Phase 3: Society

> Trading, reproduction, memory, knowledge, skills, relationships. Agents become social beings.

### 3.1 Knowledge & Discovery (`emergence-agents`, `emergence-world`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.1.1 | Implement knowledge base: known concepts set, discovery tracking (see agent-system.md §3.5) |
| ✅ | 3.1.2 | Implement seed knowledge levels 0–5 (see world-engine.md §8.2) |
| ✅ | 3.1.3 | Implement discovery adjacency map / tech tree (see world-engine.md §8.4) |
| ✅ | 3.1.4 | Implement discovery mechanics: experimentation, observation, accidental (see world-engine.md §8.3) |
| ✅ | 3.1.5 | Implement `teach` action: knowledge transfer between agents |
| ✅ | 3.1.6 | Emit `KnowledgeDiscovered` and `KnowledgeTaught` events |
| ✅ | 3.1.7 | **BUILD CHECK** — Agents discover new knowledge, teach each other |

### 3.2 Memory System (`emergence-agents`)

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.2.1 | Implement memory tiers: immediate (5 ticks), short-term (50 ticks), long-term (lifetime) (see agent-system.md §4.1) |
| ✅ | 3.2.2 | Implement memory entry structure with emotional weight and entity references (see agent-system.md §4.2) |
| ✅ | 3.2.3 | Implement memory compression: promote, summarize, or discard based on weight (see agent-system.md §4.3) |
| ✅ | 3.2.4 | Implement memory filtering for perception: relevant memories by location, agents, goals (see agent-system.md §4.4) |
| ✅ | 3.2.5 | **BUILD CHECK** — Memory persists, compresses, and appears in perception |

### 3.3 Trading & Economy

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.3.1 | Implement `trade_offer` action: propose resource exchange to co-located agent |
| ✅ | 3.3.2 | Implement `trade_accept` / `trade_reject` actions with ledger transfer |
| ✅ | 3.3.3 | Implement pending trade state in Dragonfly (offer persists until accepted/rejected/expired) |
| ✅ | 3.3.4 | Emit `TradeCompleted` and `TradeFailed` events |
| ✅ | 3.3.5 | **BUILD CHECK** — Two agents complete a trade, ledger balances |

### 3.4 Social Graph & Relationships

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.4.1 | Implement relationship scores (-1.0 to 1.0) with interaction-based updates (see agent-system.md §3.4) |
| ✅ | 3.4.2 | Include relationship context in perception (visible agents show relationship status) |
| ✅ | 3.4.3 | Implement `form_group` action: create named group with members |
| ✅ | 3.4.4 | Emit `RelationshipChanged` and `GroupFormed` events |
| ✅ | 3.4.5 | **BUILD CHECK** — Relationships evolve based on interactions |

### 3.5 Skills System

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.5.1 | Implement skill levels and XP tracking (see agent-system.md §3.6) |
| ✅ | 3.5.2 | Implement skill effects: gathering yield, building speed, teaching success |
| ✅ | 3.5.3 | Award XP on successful actions |
| ✅ | 3.5.4 | **BUILD CHECK** — Skills level up, affect action outcomes |

### 3.6 Reproduction & Lifecycle

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 3.6.1 | Implement `reproduce` action: consent check, relationship threshold, health requirements (see agent-system.md §9.2) |
| ✅ | 3.6.2 | Implement child agent creation: blended personality with mutation, inherited knowledge subset |
| ✅ | 3.6.3 | Implement maturity period: reduced action capacity for N ticks |
| ✅ | 3.6.4 | Implement aging effects: energy cap decline at 80% lifespan, movement cost at 90% (see agent-system.md §9.3) |
| ✅ | 3.6.5 | Implement population cap enforcement |
| ✅ | 3.6.6 | **BUILD CHECK** — Agents reproduce, children mature, elders decline, population caps work |

---

## Phase 4: Complexity

> Construction, infrastructure, governance, advanced economy. Civilization-level behaviors.

### 4.1 Construction System

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 4.1.1 | Implement `build` action: validate materials + knowledge, create structure (see world-engine.md §5) |
| ✅ | 4.1.2 | Implement all structure types with properties: campfire, lean-to, hut, storage, well, farm, workshop, forge, library, market, wall, bridge (see world-engine.md §5.2) |
| ✅ | 4.1.3 | Implement structure decay and `repair` action (see world-engine.md §5.3) |
| ✅ | 4.1.4 | Implement `demolish` action with 30% material salvage |
| ✅ | 4.1.5 | Implement structure effects: rest bonus, weather protection, storage, production |
| ✅ | 4.1.6 | **BUILD CHECK** — Agents build, repair, and demolish structures |

### 4.2 Advanced Actions

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 4.2.1 | Implement farming: `farm_plant`, `farm_harvest` with growth cycle |
| ✅ | 4.2.2 | Implement crafting: `craft` action at workshop with material recipes |
| ✅ | 4.2.3 | Implement mining and smelting: `mine` at rocky locations, `smelt` at forge |
| ✅ | 4.2.4 | Implement knowledge persistence: `write` to library, `read` from library |
| ✅ | 4.2.5 | **BUILD CHECK** — All advanced actions work with prerequisite knowledge |

### 4.3 Infrastructure & Routes

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 4.3.1 | Implement `improve_route` action: upgrade path type with resource cost (see world-engine.md §3.3) |
| ✅ | 4.3.2 | Implement route ACLs: allowed/denied agents, groups, toll costs (see data-schemas.md §4.7) |
| ✅ | 4.3.3 | Implement route durability and decay |
| ✅ | 4.3.4 | **BUILD CHECK** — Agents upgrade paths, set tolls, routes degrade |

### 4.4 Governance & Claims

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 4.4.1 | Implement `claim` action: take ownership of unowned structures or locations |
| ✅ | 4.4.2 | Implement `legislate` action: create rules at meeting hall with group consensus |
| ✅ | 4.4.3 | Implement `enforce` action: apply consequences for rule violations |
| ✅ | 4.4.4 | **BUILD CHECK** — Agents claim property, create and enforce rules |

### 4.5 Full Observer Dashboard

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 4.5.1 | Implement world map visualization: locations, routes, agent positions (D3.js) |
| ✅ | 4.5.2 | Implement agent inspector: deep dive into state, memory, decision history |
| ✅ | 4.5.3 | Implement economy monitor: resource flows, wealth distribution, Gini coefficient |
| ✅ | 4.5.4 | Implement social network graph: relationships, alliances, conflicts |
| ✅ | 4.5.5 | Implement timeline: scrollable event history with filtering |
| ✅ | 4.5.6 | Implement population tracker: births, deaths, population curve, average lifespan |
| ✅ | 4.5.7 | Implement discovery log and era tracker |
| ✅ | 4.5.8 | **BUILD CHECK** — All dashboard panels live with real simulation data |
| ✅ | 4.5.9 | Upgrade world map from force-directed graph to fictional continent visualization: fixed-coordinate SVG with ocean, landmass path, 3 region sub-areas (Highlands/Central Valley/Coastal Lowlands), terrain detail symbols, curved Bezier routes, pinned location nodes with glow. Remove force simulation and dragging. |
| ✅ | 4.5.10 | **BUILD CHECK** — `bun run build` and `bun run lint` pass with zero errors/warnings, continent renders correctly |

---

## Phase 5: Scale & Research

> Performance, experimentation, world events, multi-world. The platform matures.

### 5.1 Performance & Scaling

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 5.1.1 | Profile and optimize tick cycle for 100+ agents |
| ⬜ | 5.1.2 | Implement multi-runner agent partitioning (see agent-system.md §10.2) |
| ⬜ | 5.1.3 | Optimize Dragonfly key access patterns for parallel perception reads |
| ⬜ | 5.1.4 | Optimize PostgreSQL batch writes: event store, ledger, snapshots |
| ⬜ | 5.1.5 | **BUILD CHECK** — 100 agents sustain target tick rate |

### 5.2 Experiment Framework

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 5.2.1 | Implement world state save/restore (snapshot full Dragonfly + PostgreSQL state) |
| ⬜ | 5.2.2 | Implement experiment configuration: named experiments with parameter overrides |
| ⬜ | 5.2.3 | Implement A/B testing: run same starting conditions with different personality distributions |
| ⬜ | 5.2.4 | **BUILD CHECK** — Save, restore, and re-run experiments reproducibly |

### 5.3 World Events

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 5.3.1 | Implement operator event injection API (see world-engine.md §9.2) |
| ⬜ | 5.3.2 | Implement event types: natural disaster, resource boom, plague, migration pressure |
| ⬜ | 5.3.3 | Implement event types: technology gift, resource depletion, contact event |
| ⬜ | 5.3.4 | **BUILD CHECK** — Injected events affect simulation, agents respond |

### 5.4 Containment & Monitoring

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 5.4.1 | Implement seccomp profiles for agent containers |
| ⬜ | 5.4.2 | Implement escape detection: network traffic monitoring, breakout indicators (see prd.md §6.4) |
| ⬜ | 5.4.3 | Implement agent content scanning for exploitation patterns |
| ⬜ | 5.4.4 | Implement observer alerts: containment breach, population collapse, economic anomaly, first-instance milestones |
| ⬜ | 5.4.5 | **BUILD CHECK** — All containment measures active, alerts fire correctly |

---

## Phase 6: Open World & Emergence

> Open-ended action system, social construct detection, cultural emergence, operator controls. The simulation becomes a true sandbox.

### 6.1 Bounded Simulation & Operator Controls

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 6.1.1 | Implement hard simulation time limit: `max_ticks` and `max_real_time_seconds` config fields, clean shutdown with final world snapshot |
| ✅ | 6.1.2 | Implement pause/resume: operator can halt tick loop and resume, full state preserved in Dragonfly |
| ✅ | 6.1.3 | Implement variable tick speed: operator-adjustable tick interval via runtime API |
| ✅ | 6.1.4 | Implement operator REST API: Axum endpoints for pause, resume, set_speed, get_status, inject_event (authenticated, separate from agent NATS channels) |
| ✅ | 6.1.5 | Implement observer operator controls panel: React UI for simulation management (pause, speed slider, countdown timer, event injection form) |
| ✅ | 6.1.6 | Implement simulation end sequence: final world snapshot, summary report generation, observer switches to replay/analysis mode |
| ⬜ | 6.1.7 | **BUILD CHECK** — 24-hour bounded simulation runs to completion, operator controls functional from observer |

### 6.2 Smart Tick Speed & LLM Routing

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 6.2.1 | Implement routine action fast-path: rule-based decision engine for obvious survival actions (eat when starving, rest when exhausted) — bypasses LLM call entirely |
| ✅ | 6.2.2 | Implement tick complexity scoring: rate each agent's decision complexity per tick (solo survival = low, social interaction = medium, conflict/discovery = high) |
| ✅ | 6.2.3 | Implement dynamic LLM backend selection: route low-complexity decisions to cheap/fast model, high-complexity to capable model |
| ✅ | 6.2.4 | Implement night cycle optimization: sleeping/resting agents skip LLM call, tick resolves in milliseconds for idle agents |
| ⬜ | 6.2.5 | **BUILD CHECK** — Variable-speed ticks use appropriate backends, routine ticks complete in <500ms, 24-hour run achieves 30,000+ ticks |

### 6.3 Open Action System

| Status | Task | Description |
|--------|------|-------------|
| ✅ | 6.3.1 | Implement freeform action proposal: agents can submit novel actions as structured text beyond the base action catalog, with intent and target fields |
| ✅ | 6.3.2 | Implement action feasibility evaluator: World Engine evaluates novel actions for physical plausibility (location, resources, knowledge) using rule engine with LLM fallback for ambiguous cases |
| ✅ | 6.3.3 | Implement theft/stealing: take resources from co-located agent, stealth vs alertness check, emit TheftOccurred/TheftFailed events, victim notified |
| ✅ | 6.3.4 | Implement deception tracking: record ground truth vs stated information in agent messages, maintain lie history per agent, enable discovery of deceptions |
| ✅ | 6.3.5 | Implement conflict/combat system: physical confrontation between agents with health/energy consequences, aggression and skill modifiers, injury and death possible |
| ✅ | 6.3.6 | Implement diplomacy actions: propose_alliance, declare_conflict, negotiate_treaty, offer_tribute between agents or groups |
| ✅ | 6.3.7 | **BUILD CHECK** — Agents propose and execute novel actions, theft/combat/diplomacy resolve correctly with events emitted |

### 6.4 Social Construct Detection & Tracking

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 6.4.1 | Design social construct data model: generic construct with name, category (religion, governance, economic, family, cultural), adherents, founding tick, evolution history |
| ⬜ | 6.4.2 | Implement belief/narrative detection: analyze agent communications for repeated shared themes using LLM classifier, cluster into belief systems, track adherent counts |
| ⬜ | 6.4.3 | Implement governance structure tracking: detect leadership claims, voting patterns, rule declarations, authority challenges, classify government type |
| ⬜ | 6.4.4 | Implement family & relationship type tracking: detect partnerships/marriages, parent-child bonds, build lineage trees, track family units as social entities |
| ⬜ | 6.4.5 | Implement economic system detection: detect currency adoption, employment, taxation, lending, market dynamics beyond barter, classify economic model |
| ⬜ | 6.4.6 | Implement crime & justice tracking: detect norm violations (theft, deception, violence), punishment events, policing patterns (self-policing vs centralized) |
| ⬜ | 6.4.7 | Implement social construct observer panels: religion map, governance diagram, family trees, economic flow visualization, crime statistics |
| ⬜ | 6.4.8 | **BUILD CHECK** — Emergent social constructs detected, categorized, tracked over time, and visualized in observer dashboard |

### 6.5 Extended Knowledge & Innovation

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 6.5.1 | Extend tech tree through Industrial era: add ~50 knowledge items covering advanced agriculture, engineering, science, medicine, manufacturing with prerequisite chains |
| ⬜ | 6.5.2 | Implement open innovation proposals: agents can propose inventions by combining existing knowledge, World Engine evaluates prerequisite plausibility and registers new knowledge items |
| ⬜ | 6.5.3 | Implement cultural knowledge: non-mechanical discoveries (philosophy, art, music, mythology, ethics) that influence agent behavior and social cohesion but don't unlock mechanical actions |
| ⬜ | 6.5.4 | Implement technology diffusion tracking: measure how fast knowledge spreads through population, adoption curves, resistance to new ideas, knowledge hoarding |
| ⬜ | 6.5.5 | **BUILD CHECK** — Agents discover knowledge beyond Bronze Age, propose innovations, cultural knowledge emerges and spreads |

### 6.6 Advanced Communication & Reputation

| Status | Task | Description |
|--------|------|-------------|
| ⬜ | 6.6.1 | Implement private/secret communication: whisper (only target receives), conspire (group-private channel), vs public broadcast — others cannot observe private messages |
| ⬜ | 6.6.2 | Implement persuasion mechanics: agents can attempt to change others' beliefs, goals, or allegiances, success influenced by honesty trait, relationship score, and reputation |
| ⬜ | 6.6.3 | Implement reputation system: agent actions build observable reputation (generous, thief, liar, leader, warrior), visible in perception to agents with prior interaction history |
| ⬜ | 6.6.4 | Implement propaganda: persistent public declarations at locations that influence newcomers' perception of local culture and norms |
| ⬜ | 6.6.5 | **BUILD CHECK** — Private communication functional, persuasion resolves with personality influence, reputation tracks and appears in perception |

---

## Changelog Reference

See `.project/changelog.md` for detailed version history.

---

## Notes & Decisions

### Architecture Decisions
- World Engine and Agent Runtime are both Rust — single language, single build, compile-time type safety end-to-end
- Dragonfly for hot state, PostgreSQL for cold state — CQRS pattern
- NATS for messaging — cleanly separated from state layer
- Types defined once in Rust, generated to TypeScript — single source of truth
- Two-layer architecture: hard physics (World Engine) + soft culture (emergent, agent-driven)
- Bounded experiments: 24-hour real-time limit by default, full history preserved for replay
- Open action system: base mechanical catalog + freeform novel action proposals
- Operator controls: one-way command channel from Observer to World Engine via Axum REST API

### Key Technical References
- **Data Schemas:** `.project/data-schemas.md` — canonical type definitions
- **Agent System:** `.project/agent-system.md` — agent runtime specification
- **World Engine:** `.project/world-engine.md` — simulation engine design
- **Tech Stack:** `.project/tech-stack.md` — technology choices and rationale

### Known Constraints
- LLM costs scale linearly with agent count — use routine action bypass and night cycle skip to minimize calls
- Agent decision timeout (8s default) sets minimum tick duration floor
- Dragonfly memory limits agent count ceiling — snapshot and trim periodically

### Phase 6 Foundation Work Completed (2026-02-09)

The following prerequisite foundation work was completed to support Phase 6 features:

- **Config updates:** Added `simulation` section (max_ticks, max_real_time_seconds, end_condition), `operator` section (api_enabled, api_auth_token), and LLM optimization fields (routine_action_bypass, night_cycle_skip, cost_tracking) to `emergence-config.yaml`
- **Environment variables:** Updated `.env.example` to remove Ollama, clarify OpenAI as primary and Anthropic as escalation, added cost-effective model guidance, added `OPERATOR_API_TOKEN`
- **Tech stack documentation:** Updated `.project/tech-stack.md` to remove Ollama, document two-layer architecture (hard physics + soft culture), document bounded 24-hour simulation runs, update LLM backend strategy table
- **Database migrations:**
  - `0005_simulation_runs.sql` — `simulation_runs` and `operator_actions` tables with enums and indexes (supports 6.1)
  - `0006_social_constructs.sql` — `social_constructs` and `construct_memberships` tables with enums and indexes (supports 6.4)
  - `0007_deception_and_reputation.sql` — `deception_records` and `reputation_events` tables with indexes (supports 6.3, 6.6)
  - `0008_event_type_expansion.sql` — 17 new `event_type` enum values for theft, combat, deception, diplomacy, social constructs, reputation, and operator lifecycle events (supports 6.1, 6.3, 6.4, 6.6)
- **Data schemas:** Updated `.project/data-schemas.md` with SimulationRun, OperatorAction, SocialConstruct, ConstructMembership, DeceptionRecord, ReputationEvent schemas; 8 new action types (Steal, Attack, Propose, Vote, Marry, Divorce, Conspire, Pray); 17 new event types with detail schemas

---

*Last updated: 2026-02-09*
*Current Phase: Phase 5/6 — Scale & Research + Open World*
*Next Milestone: Bounded simulation & operator controls (Task 6.1.7)*
