-- Migration: Supporting Tables
-- Creates discoveries, agent_snapshots, and world_snapshots tables.
-- These are analytics/observation tables, not part of the core tick cycle,
-- but essential for the Observer Dashboard and research queries.
--
-- See: world-engine.md section 10.2, data-schemas.md sections 5, 9

-- =============================================================================
-- discoveries
-- =============================================================================
-- Knowledge milestones. Each row records the first time a piece of knowledge
-- was discovered in the simulation. This is the "tech tree" history.

CREATE TABLE discoveries (
    id              UUID        PRIMARY KEY DEFAULT uuidv7(),
    tick            BIGINT      NOT NULL,
    agent_id        UUID        REFERENCES agents(id) ON DELETE SET NULL,
    knowledge_item  TEXT        NOT NULL,
    method          TEXT        NOT NULL,
    prerequisites   JSONB       DEFAULT '[]'::JSONB,
    details         JSONB       DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: discoveries by tick, by agent, by knowledge item
CREATE INDEX idx_discoveries_tick ON discoveries(tick);
CREATE INDEX idx_discoveries_agent ON discoveries(agent_id) WHERE agent_id IS NOT NULL;
CREATE INDEX idx_discoveries_knowledge ON discoveries(knowledge_item);

-- Prevent duplicate discovery records for the same knowledge item
CREATE UNIQUE INDEX uq_discoveries_knowledge ON discoveries(knowledge_item);

-- =============================================================================
-- agent_snapshots
-- =============================================================================
-- Periodic full-state snapshots of agent state. Not written every tick -- only
-- at configurable intervals or on significant events (death, discovery, trade).
-- The full_state JSONB contains the complete AgentState from data-schemas.md.

CREATE TABLE agent_snapshots (
    id              BIGSERIAL   PRIMARY KEY,
    tick            BIGINT      NOT NULL,
    agent_id        UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    full_state      JSONB       NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: snapshots for an agent over time, snapshots at a tick
CREATE INDEX idx_agent_snapshots_agent_tick ON agent_snapshots(agent_id, tick);
CREATE INDEX idx_agent_snapshots_tick ON agent_snapshots(tick);

-- =============================================================================
-- world_snapshots
-- =============================================================================
-- End-of-tick world state summaries. One row per tick. Contains population stats,
-- economic metrics, era/season/weather, and a narrative summary.
-- This is the primary data source for the Observer Dashboard timeline.

CREATE TABLE world_snapshots (
    tick                BIGINT      PRIMARY KEY,
    era                 TEXT        NOT NULL,
    season              TEXT        NOT NULL,
    weather             TEXT        NOT NULL,
    population          INT         NOT NULL DEFAULT 0,
    births              INT         NOT NULL DEFAULT 0,
    deaths              INT         NOT NULL DEFAULT 0,
    total_resources     JSONB       NOT NULL DEFAULT '{}'::JSONB,
    wealth_distribution JSONB       NOT NULL DEFAULT '{}'::JSONB,
    trades_this_tick    INT         NOT NULL DEFAULT 0,
    discoveries_count   INT         NOT NULL DEFAULT 0,
    summary             JSONB       DEFAULT '{}'::JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: snapshots by era, recent snapshots
CREATE INDEX idx_world_snapshots_era ON world_snapshots(era);
