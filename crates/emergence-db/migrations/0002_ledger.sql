-- Migration: Central Ledger
-- Double-entry bookkeeping for all resource transfers in the simulation.
-- Every resource movement (regeneration, gathering, consumption, trade, building,
-- salvage, decay, drop, pickup) is recorded as a ledger entry.
--
-- Conservation law: sum of all quantities grouped by resource must balance
-- (world_in = agent_held + location_held + structure_held + void_consumed).
-- The LEDGER_ANOMALY alert fires if this invariant is violated.
--
-- See: data-schemas.md section 6, world-engine.md section 10.2

-- =============================================================================
-- Ledger entry type enum
-- =============================================================================

CREATE TYPE ledger_entry_type AS ENUM (
    'regeneration',     -- World -> Location (resource respawned)
    'gather',           -- Location -> Agent (agent collected resource)
    'consume',          -- Agent -> Void (agent used resource)
    'transfer',         -- Agent -> Agent (trade or gift)
    'build',            -- Agent -> Structure (construction material)
    'salvage',          -- Structure -> Agent (demolition recovery)
    'decay',            -- Structure -> Void (degradation loss)
    'drop',             -- Agent -> Location (death inventory drop)
    'pickup'            -- Location -> Agent (scavenging dropped items)
);

-- =============================================================================
-- Entity type enum (what kind of thing owns the resource)
-- =============================================================================

CREATE TYPE entity_type AS ENUM (
    'agent',
    'location',
    'structure',
    'world',
    'void'
);

-- =============================================================================
-- ledger
-- =============================================================================
-- Every row is a single resource movement. The from/to pair forms the
-- double-entry: one entity loses quantity, the other gains it.
-- quantity is always positive; direction is indicated by from/to.
-- Uses NUMERIC for precise arithmetic (no floating-point).

CREATE TABLE ledger (
    id              UUID            PRIMARY KEY DEFAULT uuidv7(),
    tick            BIGINT          NOT NULL,
    entry_type      ledger_entry_type NOT NULL,
    from_entity     UUID,
    from_entity_type entity_type,
    to_entity       UUID,
    to_entity_type  entity_type,
    resource        TEXT            NOT NULL,
    quantity        NUMERIC         NOT NULL CHECK (quantity > 0),
    reason          TEXT            NOT NULL,
    reference_id    UUID,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Primary query patterns:
-- 1. All ledger entries for a given tick (end-of-tick conservation check)
-- 2. All entries involving a specific agent (agent economic history)
-- 3. All entries of a specific type (aggregate analysis)
-- 4. All entries for a specific resource (supply chain analysis)

CREATE INDEX idx_ledger_tick ON ledger(tick);
CREATE INDEX idx_ledger_from_entity ON ledger(from_entity) WHERE from_entity IS NOT NULL;
CREATE INDEX idx_ledger_to_entity ON ledger(to_entity) WHERE to_entity IS NOT NULL;
CREATE INDEX idx_ledger_entry_type ON ledger(entry_type);
CREATE INDEX idx_ledger_resource ON ledger(resource);
CREATE INDEX idx_ledger_reference ON ledger(reference_id) WHERE reference_id IS NOT NULL;

-- Composite index for tick + resource (conservation law checks)
CREATE INDEX idx_ledger_tick_resource ON ledger(tick, resource);
