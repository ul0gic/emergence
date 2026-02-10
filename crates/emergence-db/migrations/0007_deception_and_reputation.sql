-- Migration: Deception & Reputation
-- Tracks ground-truth deception records and observable reputation events.
-- Deception records store what an agent claimed vs what was actually true,
-- enabling the simulation to track lie histories and discovery of deceptions.
-- Reputation events record observable actions that affect how other agents
-- perceive a subject.
--
-- See: data-schemas.md, build-plan.md sections 6.3, 6.6

-- =============================================================================
-- deception_records
-- =============================================================================
-- Each row is a single deception event. The claimed_info JSONB stores what the
-- deceiver told the target, and actual_truth JSONB stores what was actually true.
-- The discovered flag tracks whether the deception has been uncovered.

CREATE TABLE deception_records (
    id                  UUID        PRIMARY KEY DEFAULT uuidv7(),
    tick                BIGINT      NOT NULL,
    deceiver_id         UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    target_id           UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    deception_type      TEXT        NOT NULL,
    claimed_info        JSONB       NOT NULL DEFAULT '{}'::JSONB,
    actual_truth        JSONB       NOT NULL DEFAULT '{}'::JSONB,
    discovered          BOOLEAN     NOT NULL DEFAULT FALSE,
    discovered_at_tick  BIGINT,
    discovered_by       UUID        REFERENCES agents(id) ON DELETE SET NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: deceptions by deceiver, by target, undiscovered, by tick
CREATE INDEX idx_deception_records_deceiver ON deception_records(deceiver_id);
CREATE INDEX idx_deception_records_target ON deception_records(target_id);
CREATE INDEX idx_deception_records_tick ON deception_records(tick);
CREATE INDEX idx_deception_records_undiscovered ON deception_records(id) WHERE discovered = FALSE;
CREATE INDEX idx_deception_records_discovered_by ON deception_records(discovered_by) WHERE discovered_by IS NOT NULL;

-- =============================================================================
-- reputation_events
-- =============================================================================
-- Each row records an observable action that changes how one agent perceives
-- another. The reputation_delta is a signed numeric value (positive = good,
-- negative = bad). The context JSONB stores action-specific details.
-- Uses NUMERIC for precise delta arithmetic (no floating-point).

CREATE TABLE reputation_events (
    id                  UUID        PRIMARY KEY DEFAULT uuidv7(),
    tick                BIGINT      NOT NULL,
    subject_id          UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    observer_id         UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    action_type         TEXT        NOT NULL,
    reputation_delta    NUMERIC     NOT NULL,
    context             JSONB       NOT NULL DEFAULT '{}'::JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: reputation for a subject, reputation by observer, by tick, by action
CREATE INDEX idx_reputation_events_subject ON reputation_events(subject_id);
CREATE INDEX idx_reputation_events_observer ON reputation_events(observer_id);
CREATE INDEX idx_reputation_events_tick ON reputation_events(tick);
CREATE INDEX idx_reputation_events_action ON reputation_events(action_type);

-- Composite: reputation history for a subject as seen by a specific observer
CREATE INDEX idx_reputation_events_subject_observer ON reputation_events(subject_id, observer_id);
