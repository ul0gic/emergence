-- Migration: Core Tables
-- Creates the foundational entity tables: agents, locations, routes, structures.
-- These are the persistent representations of entities whose hot state lives in Dragonfly.
-- See: world-engine.md section 10.2, data-schemas.md sections 4.1-4.8

-- =============================================================================
-- Custom ENUM types
-- =============================================================================
-- Using TEXT columns for most enums to stay flexible during early development,
-- but creating PostgreSQL enums for the most critical, stable classifications.

CREATE TYPE path_type AS ENUM (
    'none',
    'dirt_trail',
    'worn_path',
    'road',
    'highway'
);

CREATE TYPE structure_category AS ENUM (
    'campfire',
    'lean_to',
    'basic_hut',
    'storage_pit',
    'well',
    'farm_plot',
    'workshop',
    'meeting_hall',
    'forge',
    'library',
    'market',
    'wall',
    'bridge'
);

-- =============================================================================
-- agents
-- =============================================================================
-- Immutable agent identity record. Mutable state lives in Dragonfly (hot) and
-- agent_snapshots (cold). Once created, only died_at_tick and cause_of_death
-- are ever updated.

CREATE TABLE agents (
    id              UUID        PRIMARY KEY DEFAULT uuidv7(),
    name            TEXT        NOT NULL,
    born_at_tick    BIGINT      NOT NULL,
    died_at_tick    BIGINT,
    cause_of_death  TEXT,
    parent_a        UUID        REFERENCES agents(id) ON DELETE SET NULL,
    parent_b        UUID        REFERENCES agents(id) ON DELETE SET NULL,
    generation      INT         NOT NULL DEFAULT 0,
    initial_personality JSONB   NOT NULL,
    initial_knowledge   JSONB   NOT NULL DEFAULT '[]'::JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: alive agents, agents by generation, agents by birth tick
CREATE INDEX idx_agents_alive ON agents(id) WHERE died_at_tick IS NULL;
CREATE INDEX idx_agents_born_at_tick ON agents(born_at_tick);
CREATE INDEX idx_agents_generation ON agents(generation);
CREATE INDEX idx_agents_parent_a ON agents(parent_a) WHERE parent_a IS NOT NULL;
CREATE INDEX idx_agents_parent_b ON agents(parent_b) WHERE parent_b IS NOT NULL;

-- =============================================================================
-- locations
-- =============================================================================
-- World geography nodes. Each location has a type, capacity, and a JSONB map of
-- base resource nodes (resource_type -> {available, regen_per_tick, max_capacity}).

CREATE TABLE locations (
    id              UUID        PRIMARY KEY DEFAULT uuidv7(),
    name            TEXT        NOT NULL,
    region          TEXT        NOT NULL,
    location_type   TEXT        NOT NULL,
    description     TEXT        NOT NULL DEFAULT '',
    base_resources  JSONB       NOT NULL DEFAULT '{}'::JSONB,
    capacity        INT         NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: locations by region, locations by type
CREATE INDEX idx_locations_region ON locations(region);
CREATE INDEX idx_locations_type ON locations(location_type);

-- =============================================================================
-- routes
-- =============================================================================
-- Directed weighted edges connecting locations. Bidirectional routes are stored
-- as two rows (one per direction) for simplicity in graph queries.

CREATE TABLE routes (
    id              UUID        PRIMARY KEY DEFAULT uuidv7(),
    from_location   UUID        NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    to_location     UUID        NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    cost_ticks      INT         NOT NULL,
    path_type       path_type   NOT NULL DEFAULT 'none',
    durability      INT         NOT NULL DEFAULT 100,
    max_durability  INT         NOT NULL DEFAULT 100,
    decay_per_tick  NUMERIC(6,4) NOT NULL DEFAULT 0.0,
    bidirectional   BOOLEAN     NOT NULL DEFAULT TRUE,
    acl             JSONB,
    built_by        UUID        REFERENCES agents(id) ON DELETE SET NULL,
    built_at_tick   BIGINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Prevent duplicate routes between same pair in same direction
    CONSTRAINT uq_routes_from_to UNIQUE (from_location, to_location)
);

-- Query patterns: routes from/to a location, routes by builder
CREATE INDEX idx_routes_from ON routes(from_location);
CREATE INDEX idx_routes_to ON routes(to_location);
CREATE INDEX idx_routes_builder ON routes(built_by) WHERE built_by IS NOT NULL;

-- =============================================================================
-- structures
-- =============================================================================
-- Built structures that exist at locations. Mutable fields (durability, occupants)
-- live in Dragonfly; this table holds the persistent identity and construction record.

CREATE TABLE structures (
    id              UUID                PRIMARY KEY DEFAULT uuidv7(),
    structure_type  structure_category  NOT NULL,
    subtype         TEXT,
    location_id     UUID                NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    builder         UUID                NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    owner           UUID                REFERENCES agents(id) ON DELETE SET NULL,
    built_at_tick   BIGINT              NOT NULL,
    destroyed_at_tick BIGINT,
    materials_used  JSONB               NOT NULL DEFAULT '{}'::JSONB,
    durability      INT                 NOT NULL DEFAULT 100,
    max_durability  INT                 NOT NULL DEFAULT 100,
    decay_per_tick  NUMERIC(6,4)        NOT NULL DEFAULT 0.0,
    capacity        INT                 NOT NULL DEFAULT 0,
    properties      JSONB               NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

-- Query patterns: structures at location, structures by owner, active structures
CREATE INDEX idx_structures_location ON structures(location_id);
CREATE INDEX idx_structures_owner ON structures(owner) WHERE owner IS NOT NULL;
CREATE INDEX idx_structures_builder ON structures(builder);
CREATE INDEX idx_structures_active ON structures(id) WHERE destroyed_at_tick IS NULL;
CREATE INDEX idx_structures_type ON structures(structure_type);
