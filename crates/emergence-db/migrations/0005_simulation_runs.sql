-- Migration: Simulation Runs & Operator Controls
-- Tracks bounded simulation experiments and operator interventions.
-- Each simulation run is a discrete experiment with configuration, seed,
-- start/end ticks, and a lifecycle status. Operator actions are logged
-- as an audit trail for all external interventions.
--
-- See: world-engine.md section 9, data-schemas.md, build-plan.md section 6.1

-- =============================================================================
-- Simulation run status enum
-- =============================================================================

CREATE TYPE simulation_status AS ENUM (
    'created',
    'running',
    'paused',
    'completed',
    'failed'
);

-- =============================================================================
-- Operator action type enum
-- =============================================================================

CREATE TYPE operator_action_type AS ENUM (
    'pause',
    'resume',
    'set_speed',
    'inject_event',
    'emergency_stop'
);

-- =============================================================================
-- simulation_runs
-- =============================================================================
-- Each row is a discrete simulation experiment. The config JSONB stores the
-- full emergence-config.yaml snapshot at the time the run was created, ensuring
-- reproducibility. The seed is the RNG seed for deterministic replay.

CREATE TABLE simulation_runs (
    id              UUID                PRIMARY KEY DEFAULT uuidv7(),
    name            TEXT                NOT NULL,
    description     TEXT                NOT NULL DEFAULT '',
    status          simulation_status   NOT NULL DEFAULT 'created',
    started_at_tick BIGINT,
    ended_at_tick   BIGINT,
    max_ticks       BIGINT              NOT NULL DEFAULT 0,
    config          JSONB               NOT NULL DEFAULT '{}'::JSONB,
    seed            BIGINT              NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

-- Query patterns: active runs, runs by status, recent runs
CREATE INDEX idx_simulation_runs_status ON simulation_runs(status);
CREATE INDEX idx_simulation_runs_created ON simulation_runs(created_at);

-- =============================================================================
-- operator_actions
-- =============================================================================
-- Audit trail of every operator intervention. Immutable — once written, never
-- modified. The parameters JSONB stores action-specific data (e.g., new tick
-- interval for set_speed, event payload for inject_event).

CREATE TABLE operator_actions (
    id              UUID                    PRIMARY KEY DEFAULT uuidv7(),
    run_id          UUID                    NOT NULL REFERENCES simulation_runs(id) ON DELETE CASCADE,
    tick            BIGINT                  NOT NULL,
    action_type     operator_action_type    NOT NULL,
    parameters      JSONB                   NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ             NOT NULL DEFAULT NOW()
);

-- Query patterns: actions for a run, actions by tick, actions by type
CREATE INDEX idx_operator_actions_run ON operator_actions(run_id);
CREATE INDEX idx_operator_actions_run_tick ON operator_actions(run_id, tick);
CREATE INDEX idx_operator_actions_type ON operator_actions(action_type);
