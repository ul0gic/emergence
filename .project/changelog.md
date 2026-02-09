# Emergence — Changelog

> **Document Location:** `.project/changelog.md`
>
> All notable changes to this project will be documented in this file.
> Format based on [Keep a Changelog](https://keepachangelog.com/).

---

## [Unreleased]

### Added
- Task 4.5.9 ✅: World map continent visualization — replaced force-directed graph with 6-layer fixed-coordinate SVG: ocean background, hand-crafted continent landmass with irregular coastline, 3 region sub-areas (Highlands/Central Valley/Coastal Lowlands), terrain details (mountains, trees, river, waves), quadratic Bezier curved routes, pinned location nodes with agent glow effect. Removed force simulation and node dragging.
- Task 4.5.10 ✅: Build check passed — `bun run build` and `bun run lint` zero errors/warnings
- Project documentation system (`.project/` directory)
- Product Requirements Document (`prd.md`)
- World Engine technical design (`world-engine.md`)
- Agent System specification (`agent-system.md`)
- Data Schemas reference (`data-schemas.md`)
- Technology Stack documentation (`tech-stack.md`)
- Build Plan with 134 tasks across 6 phases (`build-plan.md`)
- This changelog

### Changed
- Finalized project name: Emergence
- Committed to Rust for World Engine (over Go)
- Committed to Rust for Agent Runtime (over Python) — all LLM backends are REST, no GPU for local models, single language wins
- Committed to Dragonfly for hot state (over Redis)
- Committed to NATS for event bus (over Redis Streams)

---

## Version Guidelines

### Version Format: `MAJOR.MINOR.PATCH`

- **MAJOR**: Breaking changes or significant milestones
- **MINOR**: New features, completed phases
- **PATCH**: Bug fixes, small improvements

### Planned Milestones

| Version | Milestone | Phase | Date |
|---------|-----------|-------|------|
| 0.1.0 | Project scaffolding compiles | Phase 0 | TBD |
| 0.2.0 | World Engine tick cycle works | Phase 1 | TBD |
| 0.3.0 | Single agent proof-of-concept | Phase 1 | TBD |
| 0.4.0 | Multi-agent primitive world | Phase 2 | TBD |
| 0.5.0 | Trading and social interactions | Phase 3 | TBD |
| 0.6.0 | Knowledge and discovery system | Phase 3 | TBD |
| 0.7.0 | Construction and infrastructure | Phase 4 | TBD |
| 0.8.0 | Full observer dashboard | Phase 4 | TBD |
| 0.9.0 | Experiment framework | Phase 5 | TBD |
| 1.0.0 | First full simulation run (10+ agents, 1000+ ticks, emergent trade) | Phase 5 | TBD |

### Change Types

| Type | Description |
|------|-------------|
| **Added** | New features or capabilities |
| **Changed** | Changes to existing functionality |
| **Deprecated** | Features marked for removal |
| **Removed** | Features that were removed |
| **Fixed** | Bug fixes |
| **Security** | Security-related changes |

---

*Last updated: 2026-02-08*
