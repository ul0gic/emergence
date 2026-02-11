-- Migration: Experiment Snapshots
-- Stores full simulation state snapshots for the experiment framework.
-- Snapshots enable save/restore of experiments and A/B testing.
--
-- See: build-plan.md Phase 5.2

-- =============================================================================
-- experiment_snapshots
-- =============================================================================
-- Each row is a full simulation state snapshot at a point in time. The
-- state_blob JSONB column stores the complete world + agent state in a
-- single document for efficient save/restore.

CREATE TABLE IF NOT EXISTS experiment_snapshots (
    id              UUID            PRIMARY KEY DEFAULT uuidv7(),
    experiment_id   UUID,
    name            TEXT            NOT NULL DEFAULT '',
    description     TEXT            NOT NULL DEFAULT '',
    tick            BIGINT          NOT NULL,
    config          JSONB           NOT NULL DEFAULT '{}'::JSONB,
    state_blob      JSONB           NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Query patterns: by experiment, by tick, recent snapshots
CREATE INDEX IF NOT EXISTS idx_experiment_snapshots_experiment
    ON experiment_snapshots(experiment_id);
CREATE INDEX IF NOT EXISTS idx_experiment_snapshots_tick
    ON experiment_snapshots(tick);
CREATE INDEX IF NOT EXISTS idx_experiment_snapshots_created
    ON experiment_snapshots(created_at);
