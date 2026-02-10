# Emergence — Tech Stack

> **Document Location:** `.project/tech-stack.md`
>
> Technology choices and rationale for the Emergence simulation platform.
> All decisions are documented with reasoning and alternatives considered.

---

## Stack Overview

```
┌─────────────────────────────────────────────────────┐
│              Observer Dashboard                       │
│  React + TypeScript + D3.js + Zod                    │
├─────────────────────────────────────────────────────┤
│              Agent Runtime                            │
│  Rust + reqwest + minijinja (all LLM backends = REST)│
├─────────────────────────────────────────────────────┤
│              World Engine                             │
│  Rust (2024 edition) + Tokio + Axum                  │
├─────────────────────────────────────────────────────┤
│              Data Layer                               │
│  Dragonfly (hot state) + PostgreSQL (event store)    │
├─────────────────────────────────────────────────────┤
│              Messaging                                │
│  NATS (pub/sub + action queues)                      │
├─────────────────────────────────────────────────────┤
│              Infrastructure                           │
│  Docker Compose (rootless) + Gentoo Linux (hardened) │
└─────────────────────────────────────────────────────┘
```

---

## Core Technologies

### World Engine — Rust

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 edition | World Engine — tick cycle, state machine, ledger, validation |
| Tokio | 1.x | Async runtime — concurrent agent perception, parallel I/O |
| Axum | 0.8.x | Observer API — WebSocket tick streaming, REST queries |

**Rationale:**
- Ownership model maps directly to tick architecture — perception payloads are immutable borrows, actions transfer ownership via channels
- Zero-cost abstractions for deterministic tick timing — no GC pauses during conflict resolution at 500+ agents
- Algebraic types (enums + pattern matching) guarantee exhaustive handling of action results, rejection reasons, event types
- `sqlx` compile-time query validation — ledger inserts and event writes are type-checked before the binary exists
- `unsafe` is forbidden project-wide — the World Engine is a financial system (central ledger with conservation laws) and must be treated accordingly

**Alternatives Considered:**
- **Go** — Simpler concurrency model but weaker type safety. No compile-time guarantee that every `RejectionReason` variant is handled. GC pauses under high-throughput tick resolution.
- **TypeScript/Node** — Fast iteration but insufficient performance guarantees for tick-critical paths. No algebraic types.

---

### Agent Runtime — Rust

| Technology | Version | Purpose |
|------------|---------|---------|
| Rust | 2024 edition | Agent Runner — same workspace as World Engine |
| reqwest | 0.12.x | HTTP client for LLM API calls |
| minijinja | 2.x | Prompt template engine (templates loaded from files) |

**Rationale:**
- All LLM backends (OpenAI, Anthropic) expose REST APIs — no Python SDK required
- Single `cargo build` compiles the entire simulation (engine + runner)
- Perception and action types from `emergence-types` are compile-time checked end-to-end — no serialization bugs between runner and engine
- One Docker image, one dependency tree, one language to maintain
- Prompt templates live in files and are loaded at runtime via `minijinja` — editing a template doesn't require recompilation

**Why not Python:**
- The original plan was Python for its LLM SDK ecosystem. In practice, the agent runner is "receive JSON → build prompt → call HTTP endpoint → parse JSON → send JSON." Every LLM provider exposes a REST API. Python adds a second language, second build system, second dependency tree, and second Docker image for no material benefit.
- No GPU on the target system eliminates the local model in-process argument (the one thing that truly needed Python's ML ecosystem)
- Cheap API models (OpenAI nano/mini tier) are faster and higher quality than local models anyway

---

### Hot State — Dragonfly

| Technology | Version | Purpose |
|------------|---------|---------|
| Dragonfly | Latest stable | Hot state store — current tick world state, action queues, agent vitals |

**Rationale:**
- Redis-compatible API — drop-in replacement, same client libraries (`fred` crate)
- Multi-threaded architecture — significantly higher throughput than single-threaded Redis for concurrent agent state reads/writes
- Lower memory footprint per key than Redis
- Handles the read-heavy perception phase (all agents queried in parallel) and write-heavy resolution phase (all state updated atomically)

**Alternatives Considered:**
- **Redis** — Industry standard but single-threaded. At 200+ agents with parallel perception reads, Dragonfly's multi-threaded model wins.
- **In-memory Rust state** — Simpler but loses the clean process boundary between World Engine instances and makes horizontal scaling impossible.

---

### Event Store — PostgreSQL

| Technology | Version | Purpose |
|------------|---------|---------|
| PostgreSQL | 18+ | Persistent storage — event store, ledger, agent records, world snapshots. Native `uuidv7()` for time-ordered primary keys. |
| sqlx | 0.8.x | Compile-time checked queries, migrations, connection pooling |

**Rationale:**
- Append-only event tables with range partitioning by tick — efficient for the event sourcing architecture
- JSONB for flexible event detail payloads while keeping core fields as typed columns
- Battle-tested for financial-grade ledger operations (double-entry bookkeeping, balance verification)
- Rich query capabilities for the Observer Dashboard (aggregate stats, time-series, filtering)

**Schema Location:** Defined in `crates/emergence-db/src/migrations/` and documented in `.project/data-schemas.md`

---

### Event Bus — NATS

| Technology | Version | Purpose |
|------------|---------|---------|
| NATS | Latest stable | Pub/sub messaging — tick events, perception delivery, action submission |
| async-nats | 0.38.x | Rust NATS client |

**Rationale:**
- Lightweight, zero-config pub/sub — no broker overhead like Kafka
- Supports both pub/sub (tick broadcasts, perception delivery) and request/reply (action submission)
- Native support for subject-based routing — `tick.1205.perception.agent_042`, `tick.1205.action.agent_042`
- Low latency for the tight tick timing window (8-second agent decision deadline)

**Alternatives Considered:**
- **Redis Streams** — Works but conflates hot state and messaging concerns. NATS keeps them cleanly separated.
- **Kafka** — Overkill for this scale. Designed for massive throughput we don't need. Operational complexity.
- **RabbitMQ** — Heavier than needed. NATS is simpler for this use case.

---

### Observer Dashboard — React + TypeScript

| Technology | Version | Purpose |
|------------|---------|---------|
| React | 19.x | Dashboard UI framework |
| TypeScript | 5.x | Type safety, generated types from Rust |
| D3.js | 7.x | Data visualization — world map, social graphs, economy charts |
| Zod | 3.x | Runtime validation of WebSocket messages from World Engine |

**Rationale:**
- Types generated from Rust structs via `ts-rs` — zero drift between engine and dashboard
- Zod validates all incoming WebSocket data at runtime — catches any serialization issues
- D3.js for the heavy visualization work (social network graphs, economy Gini curves, population timelines)
- WebSocket connection to Axum backend for real-time tick streaming

**Type Generation Pipeline:**
1. Rust structs in `emergence-types` crate derive `TS` via `ts-rs`
2. `cargo test` exports TypeScript interfaces to `observer/src/types/generated/`
3. Zod schemas in `observer/src/types/schemas.ts` validate against generated interfaces
4. Zero `any` in the dashboard codebase

---

## Dependencies

### Production Dependencies (Rust — World Engine + Agent Runner)

| Package | Version | Purpose |
|---------|---------|---------|
| tokio | ^1 | Async runtime |
| axum | ^0.8 | HTTP/WebSocket server for Observer API |
| tower | ^0.5 | Middleware layer |
| tower-http | ^0.6 | CORS, tracing middleware |
| sqlx | ^0.8 | PostgreSQL driver with compile-time query checks |
| rust_decimal | ^1 | Precise decimal arithmetic for ledger |
| fred | ^9 | Dragonfly/Redis client |
| async-nats | ^0.38 | NATS pub/sub client |
| reqwest | ^0.12 | HTTP client for LLM API calls (agent runner) |
| minijinja | ^2 | Prompt template engine (agent runner) |
| serde | ^1 | Serialization framework |
| serde_json | ^1 | JSON serialization |
| ts-rs | ^10 | Rust → TypeScript type generation |
| uuid | ^1 | UUID v7 identifiers |
| chrono | ^0.4 | Timestamps |
| config | ^0.14 | YAML/TOML configuration loading |
| tracing | ^0.1 | Structured logging |
| tracing-subscriber | ^0.3 | Log output formatting |
| thiserror | ^2 | Typed error definitions |
| anyhow | ^1 | Error propagation |
| validator | ^0.20 | Input validation |

### Production Dependencies (TypeScript — Observer Dashboard)

| Package | Version | Purpose |
|---------|---------|---------|
| react | ^19 | UI framework |
| d3 | ^7 | Data visualization |
| zod | ^3 | Runtime type validation |
| @types/d3 | Latest | D3 TypeScript definitions |

### Development Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| clippy | Built-in | Rust linting (strict config — see below) |
| rustfmt | Built-in | Rust formatting |
| eslint | Latest | TypeScript linting |
| prettier | Latest | TypeScript/CSS formatting |

---

## Build & Tooling

### Build System

| Tool | Version | Purpose |
|------|---------|---------|
| Cargo | Rust toolchain | Rust workspace build, test, dependency management |
| npm/pnpm | Latest | Observer Dashboard build |
| Docker Compose | Latest | Container orchestration |

### Lint Configuration (Rust)

The World Engine enforces an extremely strict lint profile. This is a financial system — the central ledger must never panic, overflow, or lose precision.

**Key lint groups:**
- `unsafe_code = "forbid"` — No unsafe Rust anywhere
- `unwrap_used = "deny"`, `expect_used = "deny"`, `panic = "deny"` — Zero panics
- `arithmetic_side_effects = "deny"` — All arithmetic must be checked (critical for ledger)
- `cast_possible_truncation = "deny"`, `cast_sign_loss = "deny"` — No silent data loss
- `float_cmp = "deny"` — No floating-point equality comparisons
- `pedantic = "deny"`, `nursery = "deny"` — Maximum code quality

Full lint config is defined in the workspace `Cargo.toml` and documented in `.project/world-engine.md` section 14.3.

### Build Commands

```bash
# Development — World Engine + Agent Runner (single workspace)
cargo build
cargo test
cargo clippy -- -D warnings

# Run World Engine
cargo run --bin emergence

# Run Agent Runner
cargo run --bin emergence-runner

# Type generation — Rust → TypeScript
cargo test --package emergence-types export_bindings

# Development — Observer Dashboard
cd observer && npm run dev

# Production build — Observer Dashboard
cd observer && npm run build

# Full simulation — Docker Compose
docker compose up
```

---

## Architecture Patterns

### Code Organization

```
emergence/
├── .project/                  # Project documentation
│   ├── prd.md                 # Product Requirements Document
│   ├── tech-stack.md          # This file
│   ├── build-plan.md          # Task tracking with phases
│   ├── changelog.md           # Version history
│   ├── data-schemas.md        # Canonical data type definitions
│   ├── agent-system.md        # Agent runtime specification
│   └── world-engine.md        # World Engine technical design
├── Cargo.toml                 # Rust workspace root
├── crates/
│   ├── emergence-types/       # Shared types + TypeScript generation
│   ├── emergence-core/        # Tick cycle, state machine, orchestration
│   ├── emergence-world/       # Geography, environment, physics
│   ├── emergence-agents/      # Agent state, vitals, actions
│   ├── emergence-ledger/      # Central ledger, double-entry bookkeeping
│   ├── emergence-events/      # Event sourcing, snapshots
│   ├── emergence-db/          # Dragonfly + PostgreSQL data layer
│   └── emergence-runner/      # Agent Runner (LLM orchestration, separate binary)
├── src/
│   └── main.rs                # World Engine entry point — starts tick loop
├── templates/                 # Prompt templates (minijinja, editable without recompile)
├── observer/                  # React Observer Dashboard
│   ├── package.json
│   └── src/
│       └── types/
│           ├── generated/     # Auto-generated from Rust via ts-rs
│           └── schemas.ts     # Zod runtime validation
├── docker-compose.yml         # Container orchestration
└── emergence-config.yaml      # Simulation configuration
```

### Design Patterns Used

| Pattern | Where Used | Purpose |
|---------|------------|---------|
| Event Sourcing | World Engine → PostgreSQL | Every state change is an immutable event. Full history replay. |
| CQRS | Dragonfly (write) / PostgreSQL (read) | Hot state optimized for tick writes, cold state optimized for queries. |
| Double-Entry Bookkeeping | Central Ledger | Every resource transfer has matching debit/credit. Conservation law enforcement. |
| Tick-Based Simulation | Core tick cycle | Discrete time steps with deterministic phase ordering. |
| Fog of War | Perception assembly | Agents only see their local state. No omniscience. |
| Type Generation Pipeline | emergence-types → ts-rs → Zod | Single source of truth for types across Rust, TypeScript, and runtime validation. |
| Actor-like Isolation | Agent Runners | Each agent is an independent decision-maker communicating via message passing. |

---

## Environment Configuration

### Required Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `DATABASE_URL` | PostgreSQL connection string | Yes |
| `DRAGONFLY_URL` | Dragonfly connection string | Yes |
| `NATS_URL` | NATS server URL | Yes |
| `LLM_DEFAULT_API_KEY` | OpenAI API key (routine agent decisions) | Yes |
| `LLM_ESCALATION_API_KEY` | Anthropic API key (escalation decisions) | Yes |
| `OPERATOR_API_TOKEN` | Bearer token for operator REST API | No (empty = auth disabled) |
| `OBSERVER_PORT` | Dashboard port | No (default: 8080) |
| `RUST_LOG` | Logging level | No (default: info) |

### Configuration Files

| File | Purpose |
|------|---------|
| `emergence-config.yaml` | Simulation parameters (tick rate, population, economy, discovery) |
| `.env` | Environment variables (not committed) |
| `.env.example` | Template for environment variables |
| `docker-compose.yml` | Container orchestration and networking |
| `Cargo.toml` | Rust workspace config, lint rules, dependencies |

---

## External Services

### APIs & Integrations

| Service | Purpose | Required |
|---------|---------|----------|
| OpenAI API | Cost-efficient LLM for routine agent decisions (nano/mini tier models) | Yes (default backend) |
| Anthropic API | High-quality LLM for complex agent decisions (discoveries, conflicts, diplomacy) | Yes (escalation backend) |

### LLM Backend Strategy

The agent runner uses a two-tier LLM architecture optimized for cost and quality:

| Backend | Use Case | Cost | When Used |
|---------|----------|------|-----------|
| **OpenAI (nano/mini)** | Default — routine agent decisions (gathering, moving, resting, basic social) | Low | Every tick for active agents with non-trivial decisions |
| **Anthropic (Haiku)** | Escalation — discoveries, complex social interactions, governance, diplomacy, deception | Medium | Only when tick complexity scoring exceeds routine threshold |
| **Rule engine (no LLM)** | Bypass — obvious survival actions (eat when starving, rest when exhausted, sleep at night) | Free | Routine action fast-path and night cycle skip |

All backends are called via `reqwest` HTTP client from the Rust agent runner. Swapping backends is a config change (endpoint URL + API key). Configuration allows per-experiment backend selection and automatic escalation from default to escalation backend based on tick complexity scoring.

### Two-Layer Architecture

The simulation operates on two distinct layers:

1. **Hard Physics Layer (World Engine):** Deterministic, rule-based, non-negotiable. Resource conservation, ledger balancing, action validation, environmental effects, vitals, death conditions. The engine enforces these laws absolutely — no agent or LLM call can violate them.

2. **Soft Culture Layer (Emergent):** Non-deterministic, agent-driven, observed. Religion, governance, economic systems, family structures, alliances, deception, reputation. These are not programmed — they emerge from agent interactions and are detected/tracked by the social construct system.

The hard layer is the physics. The soft layer is the sociology. The engine owns the first; the agents create the second.

### Bounded Simulation Runs

Simulations run as bounded experiments with a default 24-hour real-time limit:

- **`max_real_time_seconds`** — Hard wall-clock limit (default: 86400 = 24 hours)
- **`max_ticks`** — Hard tick count limit (default: 0 = unlimited)
- **`end_condition`** — Termination trigger: `time_limit`, `extinction`, `era_reached`, or `manual`
- At shutdown, the engine writes a final world snapshot, generates a summary report, and the observer switches to replay/analysis mode
- All history is preserved in PostgreSQL for post-run analysis
- Routine action bypass and night cycle skip allow 30,000+ ticks in a 24-hour window

---

## Security Considerations

### Containment Architecture
- **Isolated VLAN** — No route to production network or internet
- **Rootless Docker** — Containers run as non-root with minimal capabilities
- **No Docker socket** inside containers — agents cannot manage containers
- **Seccomp profiles** — Block dangerous syscalls
- **Read-only filesystem** in agent containers (write only to tmpfs and world API)
- **No volume mounts** to host filesystem (except read-only config injection at startup)

### Data Protection
- Event store is append-only — no mutation of historical data
- Observer dashboard is read-only — no write path back to simulation
- Observer served on host network only, not inside container network

### Escape Detection
- Host-level monitoring for unexpected network traffic, container breakout indicators
- Agent-generated content scanned for exploitation patterns (prompt injection, encoded instructions)
- Resource consumption anomaly detection

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Tick cycle (10 agents) | < 10 seconds | End-to-end including LLM calls |
| Tick cycle (50 agents) | < 30 seconds | Parallel LLM calls |
| Tick cycle (100 agents) | < 60 seconds | Multi-runner, parallel calls |
| Perception generation | < 500ms | Per-tick, all agents |
| Resolution + persist | < 1500ms | Per-tick |
| Ledger balance check | < 100ms | Per-tick conservation law verification |
| Observer WebSocket latency | < 200ms | Tick summary delivery |

---

## Decision Log

| Date | Decision | Rationale | Alternatives Considered |
|------|----------|-----------|------------------------|
| 2026-02 | Rust for World Engine | Ownership model, zero-panic guarantees, compile-time query validation | Go (weaker type safety, GC pauses) |
| 2026-02 | Dragonfly over Redis | Multi-threaded, higher throughput for parallel agent state access | Redis (single-threaded bottleneck at scale) |
| 2026-02 | NATS over Redis Streams | Clean separation of messaging and state concerns | Redis Streams (conflates two roles), Kafka (overkill) |
| 2026-02 | Rust for Agent Runtime | Single language/build, compile-time type safety end-to-end, all LLM backends are REST anyway | Python (adds second language/build for no benefit without GPU) |
| 2026-02 | ts-rs for type generation | Single source of truth from Rust, zero manual TypeScript maintenance | Manual type sync (drift risk), protobuf (overkill) |
| 2026-02 | UUID v7 for identifiers | Time-ordered for efficient indexing, globally unique | UUID v4 (random, poor index locality), ULID (less ecosystem support) |
| 2026-02 | Event sourcing + CQRS | Full history replay, separate read/write optimization | Traditional CRUD (loses history), pure event store (loses query flexibility) |
| 2026-02 | Project name: Emergence | Dual meaning — coming into existence + complex systems from simple rules | Genesis, Petri, Terrarium, Year Zero, Substrate, Epoch |

---

*Last updated: 2026-02-08*
