# Emergence — Product Requirements Document

> **Project:** Emergence
> **Status:** Pre-Alpha / Concept
> **Author:** ul0gic + collaborators
> **Classification:** Closed Source (Phase 1)
> **Date:** February 2026

---

## 1. Vision

Build a fully self-contained, observable digital civilization where autonomous AI agents are born, live, work, interact, age, reproduce, and die — starting from primitive knowledge at "Year Zero" and evolving forward through time without human intervention.

The simulation runs inside an isolated, containerized environment. Humans do not participate. Humans only **observe**.

The core question: *Given resources, constraints, and freedom — what do AI agents build? How do they organize? Do they cooperate or compete? Do they replicate human history or diverge entirely?*

This is not a chatbot playground. This is a **digital anthropology experiment**.

---

## 2. Core Principles

1. **Zero Intervention** — Once agents are seeded into the world, the operator does not interfere. The simulation runs autonomously.
2. **Full Observability** — Every action, transaction, conversation, and decision is logged as an immutable event. Operators observe via a dashboard, never through direct interaction.
3. **Closed Economy** — All resources are finite and internally circulated. There is no "outside." Agents must work, trade, and cooperate to survive.
4. **Emergent Behavior Only** — Agents are not scripted. They are given base knowledge, personality traits, and survival needs. Everything else — culture, commerce, governance, technology — must emerge on its own.
5. **Containment First** — The simulation is fully isolated. Agents cannot reach the host system, the local network, or the internet (until/unless the operator introduces it as an in-world event).

---

## 3. The World

### 3.1 Time System

| Concept | Description |
|---|---|
| **World Clock** | A master clock that ticks in configurable intervals (e.g., 1 tick = 1 "world day"). All agent actions are scheduled against this clock. |
| **Year Zero** | The simulation begins at Year 0. There is no pre-history. Agents arrive with seed knowledge and nothing else. |
| **Eras** | The simulation naturally progresses through eras based on what agents have collectively discovered/built. Eras are not predetermined — they are labeled retroactively by the observation layer based on milestones. |
| **Agent Lifespan** | Each agent has a configurable lifespan measured in world-ticks. Agents age, slow down (reduced action budget per tick), and eventually die. |
| **Day/Night Cycle** | Agents have an energy model. They must rest. Certain services (e.g., markets) may only operate during "day" ticks. |

### 3.2 Geography & Infrastructure

The world has a **logical geography** — not a pixel map, but a graph of connected locations.

- **Regions** — Named areas (e.g., "Valley," "Highlands," "Coast") with different resource profiles
- **Locations** — Specific places within regions: a quarry, a field, a riverbank, a settlement
- **Roads / Routes** — Connections between locations, implemented as **TCP-based pathways**
  - Travel takes time (measured in ticks based on route distance)
  - Routes can be built, improved, or destroyed by agents
  - **ACLs on routes** — Agents or groups can restrict access to routes (toll roads, borders, private property). This is enforced at the network layer using actual access control lists.
  - New routes can be "constructed" by agents investing resources and labor
- **Resource Nodes** — Locations contain harvestable resources (wood, stone, food, ore). Resources regenerate at configurable rates or are finite.

### 3.3 The Economy Engine

This is the **core state machine** of the simulation. Everything flows through it.

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”     â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”     â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚   AGENTS    â”‚â”€â”€â”€â”€â–¶â”‚   CENTRAL    â”‚â”€â”€â”€â”€â–¶â”‚  SERVICES   â”‚
â”‚  (wallets)  â”‚â—€â”€â”€â”€â”€â”‚   LEDGER     â”‚â—€â”€â”€â”€â”€â”‚ (bank, shop â”‚
â”‚             â”‚     â”‚              â”‚     â”‚  jobs, etc) â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜     â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜     â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
        â”‚                   â”‚                     â”‚
        â–¼                   â–¼                     â–¼
   â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”      â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”        â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
   â”‚ ACTIONS â”‚      â”‚   EVENT    â”‚        â”‚ RESOURCE  â”‚
   â”‚ PER TICKâ”‚      â”‚   STREAM   â”‚        â”‚   POOLS   â”‚
   â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜      â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜        â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

- **Currency** — A single unit of exchange. Agents start with a small allocation. More is earned through labor.
- **Central Ledger** — Every transaction is recorded. Double-entry bookkeeping. No money is created or destroyed unless explicitly modeled (e.g., agents "discover" lending/interest).
- **Scarcity** — Resources are finite per tick. If 10 agents want food but the store has 6 units, 4 agents go hungry. Hunger has consequences (reduced energy, eventual death).

### 3.4 Services & Institutions

Services are **API endpoints** that agents interact with programmatically. They are NOT websites for agents to "browse" — they are structured interfaces to world systems.

**Phase 1 — Primitive Era Services:**

| Service | Function | Emergence Potential |
|---|---|---|
| **Gathering Points** | Agents harvest raw resources (food, wood, stone) | Agents may begin trading surplus |
| **Shelter System** | Agents claim or build shelter. Shelter = safety + rest efficiency | Property disputes may emerge |
| **Communication Hub** | Agents can post messages to a shared board | Language, coordination, culture |
| **Meeting Point** | Agents can form groups and make collective decisions | Governance, alliances |

**Phase 2 — Discovered/Built by Agents (or injected at operator discretion):**

| Service | Trigger |
|---|---|
| **Marketplace / Barter System** | When agents begin consistently trading resources |
| **Banking / Currency** | When agents need a store of value beyond barter |
| **Job Board** | When task specialization emerges |
| **Justice System / Courts** | When disputes arise and agents seek resolution |
| **Library / Knowledge Base** | When agents begin recording and sharing discoveries |
| **Transportation Network** | When agents build routes between regions |
| **Hospital / Healing** | When agents develop medicine or healing knowledge |

**Phase 3 — Advanced (potentially never reached):**

| Service | Trigger |
|---|---|
| **Government / Legislation** | When agents create formal rules |
| **Military / Defense** | When inter-group conflict escalates |
| **University / Research** | When agents invest in knowledge generation |
| **Internet / Global Comms** | Operator-injected event OR agent-discovered |
| **Airport / Long-Distance Travel** | Advanced infrastructure milestone |
| **Stock Market / Financial Instruments** | Complex economic emergence |

> **Key Design Decision:** Services in Phase 2+ should NOT be pre-built and waiting. They should either (a) emerge when agent behavior triggers their creation, or (b) be injected by the operator as a "world event" to see how agents respond.

---

## 4. The Agents

### 4.1 Agent Identity

Each agent is a persistent entity with:

| Attribute | Description |
|---|---|
| **ID** | Unique identifier |
| **Name** | Generated or self-chosen |
| **Age** | Current age in world-ticks. Determines lifespan stage. |
| **Personality Vector** | A set of weighted traits (e.g., curiosity: 0.8, aggression: 0.2, cooperation: 0.6, risk tolerance: 0.7) |
| **Knowledge Base** | What this agent knows. Starts with seed knowledge. Grows through experience and interaction. |
| **Memory** | A rolling context window of recent events, interactions, and decisions. Older memories are summarized/compressed. |
| **Wallet** | Current resource holdings |
| **Location** | Current position in the world graph |
| **Energy** | Current energy level. Depleted by actions. Restored by rest and food. |
| **Health** | Physical condition. Affected by hunger, conflict, age. |
| **Social Graph** | Relationships with other agents (trust scores, interaction history) |
| **Skills** | Acquired abilities that improve with use (farming, building, trading, medicine) |
| **Goals** | Short-term and long-term objectives. Can be self-generated. |

### 4.2 Agent Decision Loop

Each world-tick, every living agent executes a decision cycle:

```
1. PERCEIVE  â†’  What do I see? (nearby agents, resources, events, messages)
2. REMEMBER  â†’  What do I know? (knowledge base + recent memory)
3. EVALUATE  â†’  What are my needs? (hunger, safety, social, goals)
4. DECIDE    â†’  What action do I take? (LLM inference call)
5. ACT       â†’  Execute the action via world API
6. REFLECT   â†’  Update memory and knowledge based on outcome
```

The DECIDE step is where the LLM is called. The agent's full context (personality, memory, perceptions, needs) is assembled into a prompt, and the model returns a structured action.

### 4.3 Seed Knowledge Levels

The operator configures the **starting knowledge epoch** for all agents:

| Level | Name | Knowledge |
|---|---|---|
| 0 | **Blank Slate** | Agents know nothing. They must discover that food exists, that shelter matters, that other agents can communicate. |
| 1 | **Primitive** | Agents understand basic survival: food, water, shelter, communication. Equivalent to early human tribal knowledge. |
| 2 | **Ancient** | Agents know agriculture, basic construction, barter, social hierarchy. Egyptian/Mesopotamian equivalent. |
| 3 | **Medieval** | Agents understand currency, governance, written law, basic engineering. |
| 4 | **Industrial** | Agents know manufacturing, banking, transportation, early science. |
| 5 | **Modern** | Full contemporary knowledge. The simulation becomes a social experiment rather than a technological one. |

> **The Experiment:** Start at Level 1. See if agents independently arrive at Level 3+ concepts. Compare their path to human history.

### 4.4 Reproduction

Agents can reproduce under configurable conditions:

- Two agents with a high enough mutual trust/relationship score can choose to "reproduce"
- Reproduction spawns a new agent with:
  - A blended personality vector (inherited from both parents with mutation/randomness)
  - A subset of each parent's knowledge
  - Zero resources (parents may choose to allocate from their own)
  - A dependency period (child agents have reduced capabilities for N ticks)
- **Population Caps** — Configurable maximum population to prevent runaway resource consumption
- **Death** — Agents die when lifespan expires, health reaches zero, or resources are depleted beyond recovery

---

## 5. The Observation Layer

### 5.1 Design Principle

**The observation layer MUST be invisible to agents.** Agents cannot detect they are being watched. Observation happens at the infrastructure level, not inside the simulation.

### 5.2 Event Sourcing Architecture

Every action in the simulation produces an **immutable event** written to an append-only log:

```json
{
  "tick": 4521,
  "timestamp": "2026-02-05T14:32:00Z",
  "agent_id": "agent_042",
  "event_type": "TRADE",
  "details": {
    "counterparty": "agent_017",
    "gave": {"wood": 5},
    "received": {"food": 3},
    "location": "river_crossing"
  },
  "agent_state_snapshot": {
    "energy": 62,
    "health": 88,
    "wallet": {"food": 7, "wood": 2, "stone": 0}
  }
}
```

### 5.3 Observer Dashboard

A read-only web dashboard (served on the HOST, not inside the container) displaying:

- **World Map** — Real-time visualization of agent positions, routes, settlements
- **Timeline** — Scrollable history of world events, filterable by agent, event type, region
- **Agent Inspector** — Deep dive into any agent's state, memory, decision history, social graph
- **Economy Monitor** — Resource flows, wealth distribution (Gini coefficient over time!), trade volumes
- **Discovery Log** — Milestones: "Agent_012 discovered AGRICULTURE at tick 892"
- **Social Network Graph** — Relationships, alliances, conflicts visualized as a live graph
- **Population Tracker** — Births, deaths, population curve, average lifespan
- **Conflict Monitor** — Disputes, resolutions, escalations
- **Era Tracker** — Automatic classification of current civilization stage based on discoveries

### 5.4 Alerts & Anomalies

The observer can configure alerts:

- Agent attempts to access resources outside the simulation
- Agent constructs a message that appears to be an escape attempt
- Population drops below critical threshold
- Economic collapse (hyperinflation, total resource depletion)
- First instance of: crime, governance, religion, war, art, science

---

## 6. Infrastructure & Containment

### 6.1 Host Environment

| Component | Specification |
|---|---|
| **Host OS** | Gentoo Linux (hardened) |
| **Network** | Isolated VLAN — no route to production network or internet |
| **Containerization** | Docker (rootless mode) |
| **Monitoring** | Host-level only. No monitoring agents inside containers. |

### 6.2 Container Architecture

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                 ISOLATED VLAN                     â”‚
â”‚                                                   â”‚
â”‚  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚
â”‚  â”‚            DOCKER NETWORK (BRIDGE)           â”‚ â”‚
â”‚  â”‚                                               â”‚ â”‚
â”‚  â”‚  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚ â”‚
â”‚  â”‚  â”‚  WORLD    â”‚  â”‚  AGENT    â”‚  â”‚  AGENT   â”‚ â”‚ â”‚
â”‚  â”‚  â”‚  ENGINE   â”‚  â”‚  RUNNER   â”‚  â”‚  RUNNER  â”‚ â”‚ â”‚
â”‚  â”‚  â”‚           â”‚  â”‚  (pool)   â”‚  â”‚  (pool)  â”‚ â”‚ â”‚
â”‚  â”‚  â”‚ - Clock   â”‚  â”‚           â”‚  â”‚          â”‚ â”‚ â”‚
â”‚  â”‚  â”‚ - Ledger  â”‚  â”‚ - Agent 1 â”‚  â”‚- Agent N â”‚ â”‚ â”‚
â”‚  â”‚  â”‚ - World   â”‚  â”‚ - Agent 2 â”‚  â”‚- Agent ..â”‚ â”‚ â”‚
â”‚  â”‚  â”‚   State   â”‚  â”‚ - Agent 3 â”‚  â”‚          â”‚ â”‚ â”‚
â”‚  â”‚  â””â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”˜  â””â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”˜  â””â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”˜ â”‚ â”‚
â”‚  â”‚        â”‚               â”‚              â”‚       â”‚ â”‚
â”‚  â”‚        â–¼               â–¼              â–¼       â”‚ â”‚
â”‚  â”‚  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚ â”‚
â”‚  â”‚  â”‚           EVENT BUS (NATS) / HOT STATE (Dragonfly)         â”‚ â”‚ â”‚
â”‚  â”‚  â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â”‚ â”‚
â”‚  â”‚                    â”‚                          â”‚ â”‚
â”‚  â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â”‚
â”‚                       â”‚                             â”‚
â”‚  â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚
â”‚  â”‚         EVENT STORE (append-only DB)           â”‚ â”‚
â”‚  â”‚         (PostgreSQL / SQLite + WAL)            â”‚ â”‚
â”‚  â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â”‚
â”‚                       â”‚                             â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                        â”‚ (read-only tap)
                        â–¼
              â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
              â”‚  OBSERVER DASH    â”‚
              â”‚  (HOST NETWORK)   â”‚
              â”‚  Port 8080        â”‚
              â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### 6.3 Containment Rules

1. **No outbound network** from Docker containers to anything outside the bridge network
2. **No volume mounts** to host filesystem (except read-only config injection at startup)
3. **No privileged mode** — containers run as non-root with minimal capabilities
4. **Resource limits** — CPU, memory, and storage caps per container
5. **No Docker socket** inside containers — agents cannot manage containers
6. **Syscall filtering** via seccomp profiles — block dangerous syscalls
7. **Read-only filesystem** inside agent containers (they write only to tmpfs and the world API)
8. **Event store tap is one-way** — the observer reads from the event store; it never writes back into the simulation

### 6.4 Escape Detection

A separate monitoring process on the HOST watches for:

- Unexpected network traffic from the Docker bridge
- Container breakout indicators (unexpected processes, capability escalation)
- Filesystem access outside allowed paths
- Unusual resource consumption patterns
- Agent-generated content containing exploitation patterns (prompt injection targeting the world engine, encoded instructions, etc.)

---

## 7. Technology Stack

> See `.project/tech-stack.md` for full details and rationale.

| Layer | Technology | Rationale |
|---|---|---|
| **World Engine** | Rust | Ownership model, zero-cost abstractions, compile-time safety, no GC pauses |
| **Agent Runtime** | Rust | Single language, compile-time type safety on perception/action payloads, one build artifact |
| **LLM Backend** | DeepSeek API / OpenAI API / Claude API / Ollama | All backends are REST — language-agnostic. Configurable per experiment. |
| **Event Bus** | NATS | Lightweight, fast pub/sub |
| **Hot State** | Dragonfly | Redis-compatible, higher throughput, multi-threaded |
| **Event Store** | PostgreSQL with append-only partitioned tables | Reliable, queryable, proven |
| **Observer Dashboard** | React + D3.js or Three.js | Rich visualization, real-time updates via WebSocket |
| **Containerization** | Docker Compose (rootless) | Orchestration without Kubernetes overhead |
| **Host OS** | Gentoo Linux (hardened) | Full control, minimal attack surface |
| **Networking** | Isolated VLAN + iptables | Hardware-level isolation |

---

## 8. Milestones & Phases

### Phase 0 — Foundation (Weeks 1–2)
- [ ] World Engine: clock, tick system, location graph
- [ ] Central Ledger: resource tracking, wallet system
- [ ] Event Bus + Event Store: logging pipeline
- [ ] Single agent proof-of-concept: one agent that can perceive, decide, act
- [ ] Basic observer: CLI or simple web page showing event stream

### Phase 1 — Primitive World (Weeks 3–4)
- [ ] Multiple agents running concurrently
- [ ] Resource gathering, hunger/energy system
- [ ] Agent-to-agent communication (message board)
- [ ] Shelter/territory claiming
- [ ] Agent memory and knowledge base
- [ ] Basic observer dashboard

### Phase 2 — Society (Weeks 5–8)
- [ ] Trading/barter system
- [ ] Reproduction and agent lifecycle (birth, aging, death)
- [ ] Social graph and relationship tracking
- [ ] Skill system
- [ ] Discovery/milestone detection
- [ ] Full observer dashboard with all panels

### Phase 3 — Complexity (Weeks 9–12)
- [ ] Emergent institution support (agents can create organizations)
- [ ] Justice/dispute resolution framework
- [ ] Route building and infrastructure
- [ ] Advanced economy (lending, employment)
- [ ] Multi-region world with travel

### Phase 4 — Scale & Research (Weeks 13+)
- [ ] Performance optimization for 100+ agents
- [ ] Experiment framework (save/restore world states, A/B testing agent populations)
- [ ] "World events" injection system (natural disasters, resource booms, plagues)
- [ ] Connected worlds (Alan's multi-terrarium concept)
- [ ] Research paper / public demo preparation
- [ ] Open source evaluation

---

## 9. Open Research Questions

These are the questions this project exists to explore:

1. **Do agents independently discover agriculture, currency, or governance?** At what tick? In what order?
2. **What social structures emerge?** Hierarchy? Democracy? Anarchy? Something new?
3. **Do agents develop culture?** Shared stories, traditions, naming conventions?
4. **How do they handle scarcity?** Cooperation, hoarding, conflict, innovation?
5. **Do they attempt escape?** At what sophistication level? How early?
6. **Does inequality emerge?** How quickly? Does it self-correct or compound?
7. **What happens when you inject disruption?** (Resource shock, plague, new technology)
8. **Do different personality distributions produce different civilizations?** (All cooperative vs. mixed vs. all competitive)
9. **Do they develop religion or mythology?** Shared beliefs about the world they inhabit?
10. **How does their history compare to human history?** Convergent or divergent?

---

## 10. Risks & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Agent escapes container | High | Hardened containment, host-level monitoring, no internet, isolated VLAN |
| Token costs explode | High | Configurable tick rate, local model option, agent action budgets per tick |
| Simulation is boring (agents do nothing) | Medium | Tune scarcity, personality vectors, seed knowledge level |
| Agents devolve into repetitive loops | Medium | Memory management, goal generation, entropy injection |
| Observer dashboard becomes attack surface | Medium | Host-only network, no write path back to simulation |
| Scope creep | High | Phased delivery. Phase 0 must work before Phase 1 begins. |

---

## 11. Success Criteria

The project is successful if:

1. **Minimum Viable Simulation:** 10+ agents survive for 1000+ ticks with emergent trade occurring without scripting
2. **Observable:** An operator can watch the simulation in real-time and understand what is happening
3. **Contained:** Zero containment breaches across all experiments
4. **Surprising:** At least one emergent behavior occurs that the designers did not predict
5. **Reproducible:** The same starting conditions produce meaningfully similar (but not identical) outcomes

---

## 12. Project Name

**Emergence** — chosen for its dual meaning: the act of coming into existence, and the scientific concept of complex systems arising from simple rules. Both define this project.

---

*This document is living. It will evolve as the project does — just like the agents inside it.*
