-- Migration: Event Type Expansion
-- Adds new event_type enum values for Phase 6 features: theft, combat,
-- deception, diplomacy, social constructs, reputation, and operator controls.
--
-- PostgreSQL ALTER TYPE ... ADD VALUE cannot run inside a transaction block
-- when using sqlx migrations, but each ADD VALUE statement is idempotent
-- (will error if the value already exists). These are appended to the
-- existing event_type enum defined in 0003_events.sql.
--
-- See: data-schemas.md section 3.4, build-plan.md sections 6.1, 6.3, 6.4, 6.6

-- =============================================================================
-- Theft & Combat
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'theft_occurred';
ALTER TYPE event_type ADD VALUE 'theft_failed';
ALTER TYPE event_type ADD VALUE 'combat_initiated';
ALTER TYPE event_type ADD VALUE 'combat_resolved';

-- =============================================================================
-- Deception
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'deception_committed';
ALTER TYPE event_type ADD VALUE 'deception_discovered';

-- =============================================================================
-- Diplomacy
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'alliance_formed';
ALTER TYPE event_type ADD VALUE 'alliance_broken';
ALTER TYPE event_type ADD VALUE 'war_declared';
ALTER TYPE event_type ADD VALUE 'treaty_negotiated';

-- =============================================================================
-- Social Constructs
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'social_construct_formed';
ALTER TYPE event_type ADD VALUE 'social_construct_disbanded';

-- =============================================================================
-- Reputation
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'reputation_changed';

-- =============================================================================
-- Operator & Simulation Lifecycle
-- =============================================================================

ALTER TYPE event_type ADD VALUE 'operator_action';
ALTER TYPE event_type ADD VALUE 'simulation_paused';
ALTER TYPE event_type ADD VALUE 'simulation_resumed';
ALTER TYPE event_type ADD VALUE 'simulation_ended';
