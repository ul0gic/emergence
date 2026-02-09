# Emergence — Project Documentation

A structured documentation system for the Emergence simulation platform.

## What is Emergence?

A fully self-contained, observable digital civilization where autonomous AI agents are born, live, work, interact, age, reproduce, and die — starting from primitive knowledge at "Year Zero" and evolving forward through time without human intervention. Humans only observe.

## Directory Structure

```
.project/
├── prd.md             # Product Requirements Document — what we're building and why
├── tech-stack.md      # Technology choices with rationale and alternatives considered
├── build-plan.md      # Task tracking with 134 tasks across 6 phases
├── changelog.md       # Version history and milestone tracking
├── data-schemas.md    # Canonical data type definitions (single source of truth)
├── agent-system.md    # Agent runtime specification — perception, decision, memory
└── world-engine.md    # World Engine technical design — tick cycle, physics, economy
```

## Files

| File | Purpose |
|------|---------|
| `prd.md` | Vision, core principles, world design, agent design, milestones, research questions |
| `tech-stack.md` | Rust (engine + runner), Dragonfly, PostgreSQL, NATS, React — all decisions with reasoning |
| `build-plan.md` | Phased task tracking from scaffolding through scale |
| `changelog.md` | Version history tied to milestones |
| `data-schemas.md` | Every type in the simulation: agents, locations, resources, events, ledger entries |
| `agent-system.md` | How agents perceive, decide, act, remember, and die |
| `world-engine.md` | The physics engine: tick cycle, geography, economy, actions, knowledge, Rust implementation |

## Build Discipline

After completing each task:
1. Run build command (`cargo build && cargo clippy -- -D warnings`)
2. Fix any warnings/errors
3. Mark task as completed in `build-plan.md`
4. Update progress summary
5. Update `changelog.md` at milestones
6. Commit changes
