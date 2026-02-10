-- Migration: Social Constructs
-- Tracks emergent social structures detected by the social construct system.
-- Social constructs are not programmed — they emerge from agent interactions
-- and are classified by the LLM-based detection pipeline.
--
-- Categories: religion, governance, economic, family, cultural.
-- Each construct has adherents (via construct_memberships), an evolution
-- history tracking how it changed over time, and arbitrary properties.
--
-- See: data-schemas.md, build-plan.md section 6.4

-- =============================================================================
-- Social construct category enum
-- =============================================================================

CREATE TYPE social_construct_category AS ENUM (
    'religion',
    'governance',
    'economic',
    'family',
    'cultural'
);

-- =============================================================================
-- social_constructs
-- =============================================================================
-- Each row is an emergent social structure. The properties JSONB holds
-- category-specific data (e.g., deity names for religion, tax rate for
-- economic, leader_id for governance). The evolution_history JSONB is an
-- append-only array of {tick, change_description, old_value, new_value}
-- entries tracking how the construct changed over time.

CREATE TABLE social_constructs (
    id                  UUID                        PRIMARY KEY DEFAULT uuidv7(),
    name                TEXT                        NOT NULL,
    category            social_construct_category   NOT NULL,
    description         TEXT                        NOT NULL DEFAULT '',
    founded_at_tick     BIGINT                      NOT NULL,
    founded_by          UUID                        REFERENCES agents(id) ON DELETE SET NULL,
    disbanded_at_tick   BIGINT,
    adherent_count      INT                         NOT NULL DEFAULT 0,
    properties          JSONB                       NOT NULL DEFAULT '{}'::JSONB,
    evolution_history   JSONB                       NOT NULL DEFAULT '[]'::JSONB,
    created_at          TIMESTAMPTZ                 NOT NULL DEFAULT NOW()
);

-- Query patterns: active constructs, by category, by founder, by founding tick
CREATE INDEX idx_social_constructs_category ON social_constructs(category);
CREATE INDEX idx_social_constructs_active ON social_constructs(id) WHERE disbanded_at_tick IS NULL;
CREATE INDEX idx_social_constructs_founded_by ON social_constructs(founded_by) WHERE founded_by IS NOT NULL;
CREATE INDEX idx_social_constructs_founded_tick ON social_constructs(founded_at_tick);

-- =============================================================================
-- construct_memberships
-- =============================================================================
-- Junction table linking agents to social constructs. An agent can belong to
-- multiple constructs simultaneously. The role field describes their position
-- within the construct (e.g., "leader", "member", "priest", "elder").

CREATE TABLE construct_memberships (
    id              UUID        PRIMARY KEY DEFAULT uuidv7(),
    construct_id    UUID        NOT NULL REFERENCES social_constructs(id) ON DELETE CASCADE,
    agent_id        UUID        NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    joined_at_tick  BIGINT      NOT NULL,
    left_at_tick    BIGINT,
    role            TEXT        NOT NULL DEFAULT 'member',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Query patterns: members of a construct, constructs for an agent, active memberships
CREATE INDEX idx_construct_memberships_construct ON construct_memberships(construct_id);
CREATE INDEX idx_construct_memberships_agent ON construct_memberships(agent_id);
CREATE INDEX idx_construct_memberships_active ON construct_memberships(id) WHERE left_at_tick IS NULL;

-- Prevent duplicate active memberships (same agent in same construct)
CREATE UNIQUE INDEX uq_construct_memberships_active ON construct_memberships(construct_id, agent_id) WHERE left_at_tick IS NULL;
