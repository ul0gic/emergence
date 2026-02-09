-- Migration: Events Table (Partitioned)
-- Append-only event store. Every state change in the simulation produces an
-- immutable event. Events are the source of truth -- state can be reconstructed
-- by replaying events from tick 0.
--
-- Partitioned by tick range for efficient querying and archival. Each partition
-- covers 10,000 ticks. New partitions should be created as the simulation
-- progresses (managed by the World Engine or a maintenance job).
--
-- See: world-engine.md section 10.2, data-schemas.md section 5

-- =============================================================================
-- Event type enum
-- =============================================================================

CREATE TYPE event_type AS ENUM (
    'tick_start',
    'tick_end',
    'agent_born',
    'agent_died',
    'action_submitted',
    'action_succeeded',
    'action_rejected',
    'resource_gathered',
    'resource_consumed',
    'trade_completed',
    'trade_failed',
    'structure_built',
    'structure_destroyed',
    'structure_repaired',
    'route_improved',
    'location_discovered',
    'knowledge_discovered',
    'knowledge_taught',
    'message_sent',
    'group_formed',
    'relationship_changed',
    'weather_changed',
    'season_changed',
    'ledger_anomaly'
);

-- =============================================================================
-- events (partitioned parent table)
-- =============================================================================
-- The parent table defines the schema. Rows are automatically routed to the
-- correct child partition based on the tick value.
--
-- id uses BIGSERIAL within each partition. The combination of (id, tick) is
-- globally unique due to partitioning by tick range.

CREATE TABLE events (
    id                      BIGSERIAL,
    tick                    BIGINT          NOT NULL,
    event_type              event_type      NOT NULL,
    agent_id                UUID,
    location_id             UUID,
    details                 JSONB           NOT NULL DEFAULT '{}'::JSONB,
    agent_state_snapshot    JSONB,
    world_context           JSONB,
    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    -- Partitioned tables require the partition key in the primary key
    PRIMARY KEY (id, tick)
) PARTITION BY RANGE (tick);

-- =============================================================================
-- Initial partitions
-- =============================================================================
-- Each partition covers 10,000 ticks. The World Engine will need to create
-- new partitions before the simulation reaches the boundary.

CREATE TABLE events_tick_0_10k
    PARTITION OF events FOR VALUES FROM (0) TO (10000);

CREATE TABLE events_tick_10k_20k
    PARTITION OF events FOR VALUES FROM (10000) TO (20000);

CREATE TABLE events_tick_20k_30k
    PARTITION OF events FOR VALUES FROM (20000) TO (30000);

-- =============================================================================
-- Indexes
-- =============================================================================
-- Indexes on partitioned tables are automatically created on each partition.

CREATE INDEX idx_events_tick ON events(tick);
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_agent ON events(agent_id) WHERE agent_id IS NOT NULL;
CREATE INDEX idx_events_location ON events(location_id) WHERE location_id IS NOT NULL;

-- Composite: find all events for an agent in a tick range (agent history view)
CREATE INDEX idx_events_agent_tick ON events(agent_id, tick) WHERE agent_id IS NOT NULL;

-- Composite: find all events of a type in a tick range (analytics)
CREATE INDEX idx_events_type_tick ON events(event_type, tick);
