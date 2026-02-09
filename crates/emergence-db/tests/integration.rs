//! Integration tests for the `emergence-db` data layer.
//!
//! These tests require live Docker services (Dragonfly and `PostgreSQL`).
//! Run with:
//!
//! ```bash
//! docker compose up -d
//! cargo test -p emergence-db -- --ignored
//! docker compose down
//! ```
//!
//! All tests are marked `#[ignore]` so they are skipped during normal
//! `cargo test` runs.

// Integration tests use expect/unwrap extensively for clarity -- panicking
// on failure is the correct behavior in test code.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::items_after_statements,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::indexing_slicing
)]

use chrono::Utc;
use emergence_db::{
    AgentSnapshotRow, DbError, DragonflyPool, EventRow, EventStore, LedgerRow, LedgerStore,
    PostgresConfig, PostgresPool, SnapshotStore, WorldSnapshotRow,
};
use emergence_types::{
    AgentId, AgentStateSnapshot, EntityType, Event, EventId, EventType, LedgerEntry,
    LedgerEntryId, LedgerEntryType, LocationId, Resource, WorldContext,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `PostgreSQL` connection URL for the local Docker instance.
const POSTGRES_URL: &str = "postgresql://emergence:emergence_dev_2026@localhost:5432/emergence";

/// Dragonfly connection URL for the local Docker instance.
const DRAGONFLY_URL: &str = "redis://localhost:6379";

// =============================================================================
// Helper: connect to PostgreSQL and run migrations
// =============================================================================

async fn setup_postgres() -> PostgresPool {
    let pool = PostgresPool::connect_url(POSTGRES_URL)
        .await
        .expect("Failed to connect to PostgreSQL -- is Docker running?");
    pool.run_migrations()
        .await
        .expect("Failed to run migrations");
    pool
}

// =============================================================================
// Dragonfly Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_connect_and_ping() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");

    // Verify we can write and read back
    pool.set_world_tick(42)
        .await
        .expect("Failed to set world tick");
    let tick = pool
        .get_world_tick()
        .await
        .expect("Failed to get world tick");
    assert_eq!(tick, 42);

    // Cleanup
    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_agent_state_roundtrip() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestState {
        name: String,
        energy: u32,
    }

    let agent_id = Uuid::now_v7();
    let state = TestState {
        name: "Alice".to_owned(),
        energy: 80,
    };

    pool.set_agent_state(agent_id, &state)
        .await
        .expect("Failed to set agent state");

    let retrieved: TestState = pool
        .get_agent_state(agent_id)
        .await
        .expect("Failed to get agent state");
    assert_eq!(retrieved, state);

    pool.delete_agent_state(agent_id)
        .await
        .expect("Failed to delete agent state");

    let result: Result<TestState, DbError> = pool.get_agent_state(agent_id).await;
    assert!(result.is_err(), "Expected KeyNotFound after deletion");

    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_location_state_roundtrip() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestLocation {
        name: String,
        occupants: Vec<String>,
    }

    let location_id = Uuid::now_v7();
    let loc = TestLocation {
        name: "Riverside".to_owned(),
        occupants: vec!["Alice".to_owned(), "Bob".to_owned()],
    };

    pool.set_location_state(location_id, &loc)
        .await
        .expect("Failed to set location state");

    let retrieved: TestLocation = pool
        .get_location_state(location_id)
        .await
        .expect("Failed to get location state");
    assert_eq!(retrieved, loc);

    pool.delete_location_state(location_id)
        .await
        .expect("Failed to delete location state");

    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_location_messages() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestMessage {
        from: String,
        content: String,
    }

    let location_id = Uuid::now_v7();

    let msg1 = TestMessage {
        from: "Alice".to_owned(),
        content: "Hello!".to_owned(),
    };
    let msg2 = TestMessage {
        from: "Bob".to_owned(),
        content: "Hi there!".to_owned(),
    };

    pool.push_location_message(location_id, &msg1)
        .await
        .expect("Failed to push message 1");
    pool.push_location_message(location_id, &msg2)
        .await
        .expect("Failed to push message 2");

    let messages: Vec<TestMessage> = pool
        .get_location_messages(location_id)
        .await
        .expect("Failed to get messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], msg1);
    assert_eq!(messages[1], msg2);

    pool.clear_location_messages(location_id)
        .await
        .expect("Failed to clear messages");

    let empty: Vec<TestMessage> = pool
        .get_location_messages(location_id)
        .await
        .expect("Failed to get messages after clear");
    assert!(empty.is_empty());

    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_trade_roundtrip() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestTrade {
        offerer: String,
        receiver: String,
        amount: u32,
    }

    let trade_id = Uuid::now_v7();
    let trade = TestTrade {
        offerer: "Alice".to_owned(),
        receiver: "Bob".to_owned(),
        amount: 10,
    };

    pool.set_trade(trade_id, &trade)
        .await
        .expect("Failed to set trade");

    let retrieved: TestTrade = pool
        .get_trade(trade_id)
        .await
        .expect("Failed to get trade");
    assert_eq!(retrieved, trade);

    pool.delete_trade(trade_id)
        .await
        .expect("Failed to delete trade");

    let result: Result<TestTrade, DbError> = pool.get_trade(trade_id).await;
    assert!(result.is_err(), "Expected KeyNotFound after deletion");

    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_alive_dead_agents() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    let agent1 = Uuid::now_v7();
    let agent2 = Uuid::now_v7();

    pool.add_alive_agent(agent1)
        .await
        .expect("Failed to add agent 1");
    pool.add_alive_agent(agent2)
        .await
        .expect("Failed to add agent 2");

    let alive = pool
        .get_alive_agents()
        .await
        .expect("Failed to get alive agents");
    assert_eq!(alive.len(), 2);
    assert!(alive.contains(&agent1));
    assert!(alive.contains(&agent2));

    pool.mark_agent_dead(agent1)
        .await
        .expect("Failed to mark agent dead");

    let alive_after = pool
        .get_alive_agents()
        .await
        .expect("Failed to get alive agents after death");
    assert_eq!(alive_after.len(), 1);
    assert!(alive_after.contains(&agent2));
    assert!(!alive_after.contains(&agent1));

    pool.flush_all().await.expect("Failed to flush");
}

#[tokio::test]
#[ignore = "requires live Dragonfly instance (docker compose up -d)"]
async fn dragonfly_world_clock_roundtrip() {
    let pool = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    pool.flush_all().await.expect("Failed to flush");

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestClock {
        tick: u64,
        season: String,
        time_of_day: String,
    }

    let clock = TestClock {
        tick: 100,
        season: "spring".to_owned(),
        time_of_day: "morning".to_owned(),
    };

    pool.set_world_clock(&clock)
        .await
        .expect("Failed to set world clock");

    let retrieved: TestClock = pool
        .get_world_clock()
        .await
        .expect("Failed to get world clock");
    assert_eq!(retrieved, clock);

    pool.flush_all().await.expect("Failed to flush");
}

// =============================================================================
// PostgreSQL Connection Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn postgres_connect_and_migrate() {
    let pool = setup_postgres().await;

    // Verify we can access the pool
    let pg_pool = pool.pool();
    let row: (i64,) = sqlx::query_as("SELECT 1::BIGINT")
        .fetch_one(pg_pool)
        .await
        .expect("Failed to execute test query");
    assert_eq!(row.0, 1);

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn postgres_config_builder() {
    let config = PostgresConfig::new(POSTGRES_URL)
        .with_max_connections(5)
        .with_connect_timeout(std::time::Duration::from_secs(10))
        .with_idle_timeout(std::time::Duration::from_secs(60));

    let pool = PostgresPool::connect(&config)
        .await
        .expect("Failed to connect with custom config");

    let pg_pool = pool.pool();
    let row: (i64,) = sqlx::query_as("SELECT 1::BIGINT")
        .fetch_one(pg_pool)
        .await
        .expect("Failed to execute test query");
    assert_eq!(row.0, 1);

    pool.close().await;
}

// =============================================================================
// Event Store Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn event_store_batch_insert_and_query() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // Clean up any previous test data in the test tick range
    sqlx::query("DELETE FROM events WHERE tick = 9999")
        .execute(pg)
        .await
        .expect("Failed to clean up test events");

    let store = EventStore::new(pg);

    let agent_id = AgentId::new();
    let location_id = LocationId::new();
    let now = Utc::now();

    let world_ctx = WorldContext {
        tick: 9999,
        era: emergence_types::Era::Primitive,
        season: emergence_types::Season::Spring,
        weather: emergence_types::Weather::Clear,
        population: 10,
    };

    let events = vec![
        Event {
            id: EventId::new(),
            tick: 9999,
            event_type: EventType::TickStart,
            agent_id: None,
            location_id: None,
            details: serde_json::json!({"message": "tick started"}),
            agent_state_snapshot: None,
            world_context: world_ctx.clone(),
            created_at: now,
        },
        Event {
            id: EventId::new(),
            tick: 9999,
            event_type: EventType::ResourceGathered,
            agent_id: Some(agent_id),
            location_id: Some(location_id),
            details: serde_json::json!({
                "resource": "water",
                "quantity": 5
            }),
            agent_state_snapshot: Some(AgentStateSnapshot {
                energy: 80,
                health: 100,
                hunger: 20,
                age: 50,
                location_id,
                inventory_summary: std::collections::BTreeMap::new(),
            }),
            world_context: world_ctx.clone(),
            created_at: now,
        },
        Event {
            id: EventId::new(),
            tick: 9999,
            event_type: EventType::TickEnd,
            agent_id: None,
            location_id: None,
            details: serde_json::json!({"message": "tick ended"}),
            agent_state_snapshot: None,
            world_context: world_ctx,
            created_at: now,
        },
    ];

    store
        .batch_insert(&events)
        .await
        .expect("Failed to batch insert events");

    let rows: Vec<EventRow> = store
        .get_events_by_tick(9999)
        .await
        .expect("Failed to query events by tick");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].event_type, "tick_start");
    assert_eq!(rows[1].event_type, "resource_gathered");
    assert_eq!(rows[2].event_type, "tick_end");

    // Query by agent
    let agent_rows: Vec<EventRow> = store
        .get_events_by_agent(agent_id.into_inner(), 9999, 10000)
        .await
        .expect("Failed to query events by agent");
    assert_eq!(agent_rows.len(), 1);
    assert_eq!(agent_rows[0].event_type, "resource_gathered");

    // Clean up
    sqlx::query("DELETE FROM events WHERE tick = 9999")
        .execute(pg)
        .await
        .expect("Failed to clean up test events");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn event_store_empty_batch() {
    let pool = setup_postgres().await;
    let pg = pool.pool();
    let store = EventStore::new(pg);

    // Empty batch should succeed without error
    store
        .batch_insert(&[])
        .await
        .expect("Empty batch should not fail");

    pool.close().await;
}

// =============================================================================
// Ledger Store Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn ledger_store_batch_insert_and_query() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // Clean up any previous test data
    sqlx::query("DELETE FROM ledger WHERE tick = 9998")
        .execute(pg)
        .await
        .expect("Failed to clean up test ledger entries");

    let store = LedgerStore::new(pg);

    let agent_id = Uuid::now_v7();
    let location_id = Uuid::now_v7();
    let now = Utc::now();

    let entries = vec![
        LedgerEntry {
            id: LedgerEntryId::new(),
            tick: 9998,
            entry_type: LedgerEntryType::Regeneration,
            from_entity: None,
            from_entity_type: Some(EntityType::World),
            to_entity: Some(location_id),
            to_entity_type: Some(EntityType::Location),
            resource: Resource::Water,
            quantity: Decimal::new(10, 0),
            reason: "REGEN".to_owned(),
            reference_id: None,
            created_at: now,
        },
        LedgerEntry {
            id: LedgerEntryId::new(),
            tick: 9998,
            entry_type: LedgerEntryType::Gather,
            from_entity: Some(location_id),
            from_entity_type: Some(EntityType::Location),
            to_entity: Some(agent_id),
            to_entity_type: Some(EntityType::Agent),
            resource: Resource::Water,
            quantity: Decimal::new(5, 0),
            reason: "GATHER".to_owned(),
            reference_id: None,
            created_at: now,
        },
        LedgerEntry {
            id: LedgerEntryId::new(),
            tick: 9998,
            entry_type: LedgerEntryType::Consume,
            from_entity: Some(agent_id),
            from_entity_type: Some(EntityType::Agent),
            to_entity: None,
            to_entity_type: Some(EntityType::Void),
            resource: Resource::Water,
            quantity: Decimal::new(2, 0),
            reason: "DRINK".to_owned(),
            reference_id: None,
            created_at: now,
        },
    ];

    store
        .batch_insert(&entries)
        .await
        .expect("Failed to batch insert ledger entries");

    let rows: Vec<LedgerRow> = store
        .get_entries_by_tick(9998)
        .await
        .expect("Failed to query ledger by tick");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].entry_type, "regeneration");
    assert_eq!(rows[1].entry_type, "gather");
    assert_eq!(rows[2].entry_type, "consume");

    // Verify quantities
    assert_eq!(rows[0].quantity, Decimal::new(10, 0));
    assert_eq!(rows[1].quantity, Decimal::new(5, 0));
    assert_eq!(rows[2].quantity, Decimal::new(2, 0));

    // Query by entity (the agent should appear in gather and consume)
    let entity_rows: Vec<LedgerRow> = store
        .get_entries_by_entity(agent_id)
        .await
        .expect("Failed to query ledger by entity");
    assert_eq!(entity_rows.len(), 2);

    // Clean up
    sqlx::query("DELETE FROM ledger WHERE tick = 9998")
        .execute(pg)
        .await
        .expect("Failed to clean up test ledger entries");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn ledger_store_empty_batch() {
    let pool = setup_postgres().await;
    let pg = pool.pool();
    let store = LedgerStore::new(pg);

    store
        .batch_insert(&[])
        .await
        .expect("Empty batch should not fail");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn ledger_store_custom_batch_size() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    sqlx::query("DELETE FROM ledger WHERE tick = 9997")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    // Use a very small batch size to test chunking
    let store = LedgerStore::new(pg).with_batch_size(2);

    let now = Utc::now();
    let entries: Vec<LedgerEntry> = (0..5)
        .map(|i| LedgerEntry {
            id: LedgerEntryId::new(),
            tick: 9997,
            entry_type: LedgerEntryType::Regeneration,
            from_entity: None,
            from_entity_type: Some(EntityType::World),
            to_entity: Some(Uuid::now_v7()),
            to_entity_type: Some(EntityType::Location),
            resource: Resource::Wood,
            quantity: Decimal::new(i64::from(i) + 1, 0),
            reason: format!("REGEN_{i}"),
            reference_id: None,
            created_at: now,
        })
        .collect();

    store
        .batch_insert(&entries)
        .await
        .expect("Failed to batch insert with custom size");

    let rows: Vec<LedgerRow> = store
        .get_entries_by_tick(9997)
        .await
        .expect("Failed to query");
    assert_eq!(rows.len(), 5);

    sqlx::query("DELETE FROM ledger WHERE tick = 9997")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    pool.close().await;
}

// =============================================================================
// Snapshot Store Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn snapshot_store_world_snapshot_roundtrip() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // Clean up
    sqlx::query("DELETE FROM world_snapshots WHERE tick = 9996")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    let store = SnapshotStore::new(pg);

    let total_resources = serde_json::json!({
        "water": 1000,
        "wood": 500,
        "stone": 200
    });
    let wealth_dist = serde_json::json!({
        "gini": "0.35",
        "top_10_pct": "0.50"
    });
    let summary = serde_json::json!({
        "narrative": "A peaceful tick with active trade."
    });

    store
        .insert_world_snapshot(
            9996,
            "primitive",
            "spring",
            "clear",
            10,
            1,
            0,
            &total_resources,
            &wealth_dist,
            3,
            0,
            &summary,
        )
        .await
        .expect("Failed to insert world snapshot");

    let row: Option<WorldSnapshotRow> = store
        .get_world_snapshot(9996)
        .await
        .expect("Failed to query world snapshot");
    assert!(row.is_some());

    let snap = row.expect("snapshot should exist");
    assert_eq!(snap.tick, 9996);
    assert_eq!(snap.era, "primitive");
    assert_eq!(snap.season, "spring");
    assert_eq!(snap.weather, "clear");
    assert_eq!(snap.population, 10);
    assert_eq!(snap.births, 1);
    assert_eq!(snap.deaths, 0);
    assert_eq!(snap.trades_this_tick, 3);
    assert_eq!(snap.discoveries_count, 0);

    // Test upsert (ON CONFLICT UPDATE)
    store
        .insert_world_snapshot(
            9996,
            "tribal",
            "summer",
            "rain",
            12,
            2,
            1,
            &total_resources,
            &wealth_dist,
            5,
            1,
            &summary,
        )
        .await
        .expect("Upsert should succeed");

    let updated: Option<WorldSnapshotRow> = store
        .get_world_snapshot(9996)
        .await
        .expect("Failed to query after upsert");
    let snap2 = updated.expect("snapshot should exist after upsert");
    assert_eq!(snap2.era, "tribal");
    assert_eq!(snap2.population, 12);
    assert_eq!(snap2.births, 2);
    assert_eq!(snap2.deaths, 1);

    // Clean up
    sqlx::query("DELETE FROM world_snapshots WHERE tick = 9996")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn snapshot_store_recent_world_snapshots() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // Clean up test range
    sqlx::query("DELETE FROM world_snapshots WHERE tick BETWEEN 9990 AND 9995")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    let store = SnapshotStore::new(pg);

    let empty = serde_json::json!({});

    for tick in 9990..=9995 {
        store
            .insert_world_snapshot(
                tick, "primitive", "spring", "clear", 10, 0, 0, &empty, &empty, 0, 0, &empty,
            )
            .await
            .expect("Failed to insert snapshot");
    }

    let recent: Vec<WorldSnapshotRow> = store
        .get_recent_world_snapshots(3)
        .await
        .expect("Failed to query recent snapshots");
    assert!(recent.len() >= 3);
    // They should be in descending tick order
    assert!(recent[0].tick >= recent[1].tick);
    assert!(recent[1].tick >= recent[2].tick);

    // Clean up
    sqlx::query("DELETE FROM world_snapshots WHERE tick BETWEEN 9990 AND 9995")
        .execute(pg)
        .await
        .expect("Failed to clean up");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn snapshot_store_agent_snapshot_roundtrip() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // We need an agent in the agents table for the FK constraint
    let agent_uuid = Uuid::now_v7();
    sqlx::query(
        r"INSERT INTO agents (id, name, born_at_tick, generation, initial_personality)
          VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(agent_uuid)
    .bind("TestAgent")
    .bind(0_i64)
    .bind(0_i32)
    .bind(serde_json::json!({
        "curiosity": "0.5",
        "cooperation": "0.5",
        "aggression": "0.3",
        "risk_tolerance": "0.4",
        "industriousness": "0.6",
        "sociability": "0.5",
        "honesty": "0.7",
        "loyalty": "0.6"
    }))
    .execute(pg)
    .await
    .expect("Failed to insert test agent");

    let store = SnapshotStore::new(pg);

    let full_state = serde_json::json!({
        "energy": 80,
        "health": 100,
        "hunger": 20,
        "age": 10,
        "inventory": {"water": 5}
    });

    store
        .insert_agent_snapshot(100, agent_uuid, &full_state)
        .await
        .expect("Failed to insert agent snapshot");

    let latest: Option<AgentSnapshotRow> = store
        .get_latest_agent_snapshot(agent_uuid)
        .await
        .expect("Failed to query latest agent snapshot");
    assert!(latest.is_some());
    let snap = latest.expect("snapshot should exist");
    assert_eq!(snap.tick, 100);
    assert_eq!(snap.agent_id, agent_uuid);
    assert_eq!(snap.full_state["energy"], 80);

    // Insert more snapshots and query range
    store
        .insert_agent_snapshot(
            200,
            agent_uuid,
            &serde_json::json!({"energy": 60, "age": 110}),
        )
        .await
        .expect("Failed to insert second snapshot");

    store
        .insert_agent_snapshot(
            300,
            agent_uuid,
            &serde_json::json!({"energy": 40, "age": 210}),
        )
        .await
        .expect("Failed to insert third snapshot");

    let range: Vec<AgentSnapshotRow> = store
        .get_agent_snapshots(agent_uuid, 100, 300)
        .await
        .expect("Failed to query agent snapshots by range");
    assert_eq!(range.len(), 2); // ticks 100, 200 (300 excluded since < 300 means up-to-not-including)

    // Latest should be tick 300
    let latest2: Option<AgentSnapshotRow> = store
        .get_latest_agent_snapshot(agent_uuid)
        .await
        .expect("Failed to query latest");
    assert_eq!(latest2.expect("should exist").tick, 300);

    // Clean up (snapshots first due to FK)
    sqlx::query("DELETE FROM agent_snapshots WHERE agent_id = $1")
        .bind(agent_uuid)
        .execute(pg)
        .await
        .expect("Failed to clean up snapshots");
    sqlx::query("DELETE FROM agents WHERE id = $1")
        .bind(agent_uuid)
        .execute(pg)
        .await
        .expect("Failed to clean up agent");

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires live PostgreSQL instance (docker compose up -d)"]
async fn snapshot_store_batch_insert_agent_snapshots() {
    let pool = setup_postgres().await;
    let pg = pool.pool();

    // Create test agent
    let agent_uuid = Uuid::now_v7();
    sqlx::query(
        r"INSERT INTO agents (id, name, born_at_tick, generation, initial_personality)
          VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(agent_uuid)
    .bind("BatchAgent")
    .bind(0_i64)
    .bind(0_i32)
    .bind(serde_json::json!({
        "curiosity": "0.5",
        "cooperation": "0.5",
        "aggression": "0.3",
        "risk_tolerance": "0.4",
        "industriousness": "0.6",
        "sociability": "0.5",
        "honesty": "0.7",
        "loyalty": "0.6"
    }))
    .execute(pg)
    .await
    .expect("Failed to insert test agent");

    let store = SnapshotStore::new(pg);

    let snapshots: Vec<(u64, Uuid, serde_json::Value)> = (0u64..5)
        .map(|i| {
            (
                i * 100,
                agent_uuid,
                serde_json::json!({"energy": 100 - i * 10, "tick": i * 100}),
            )
        })
        .collect();

    store
        .batch_insert_agent_snapshots(&snapshots)
        .await
        .expect("Failed to batch insert agent snapshots");

    let all: Vec<AgentSnapshotRow> = store
        .get_agent_snapshots(agent_uuid, 0, 500)
        .await
        .expect("Failed to query");
    assert_eq!(all.len(), 5);

    // Clean up
    sqlx::query("DELETE FROM agent_snapshots WHERE agent_id = $1")
        .bind(agent_uuid)
        .execute(pg)
        .await
        .expect("Failed to clean up snapshots");
    sqlx::query("DELETE FROM agents WHERE id = $1")
        .bind(agent_uuid)
        .execute(pg)
        .await
        .expect("Failed to clean up agent");

    pool.close().await;
}

// =============================================================================
// Cross-Store Tests
// =============================================================================

#[tokio::test]
#[ignore = "requires live PostgreSQL and Dragonfly (docker compose up -d)"]
async fn full_tick_data_flow() {
    // This test simulates a minimal end-of-tick flush:
    // 1. Write hot state to Dragonfly
    // 2. Flush events and ledger entries to PostgreSQL
    // 3. Write world snapshot
    // 4. Advance tick counter

    let df = DragonflyPool::connect(DRAGONFLY_URL)
        .await
        .expect("Failed to connect to Dragonfly");
    df.flush_all().await.expect("Failed to flush Dragonfly");

    let pg_pool = setup_postgres().await;
    let pg = pg_pool.pool();

    // Clean up test tick
    let test_tick: u64 = 9900;
    sqlx::query("DELETE FROM events WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean events");
    sqlx::query("DELETE FROM ledger WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean ledger");
    sqlx::query("DELETE FROM world_snapshots WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean snapshots");

    // Phase 1: Set hot state in Dragonfly
    df.set_world_tick(test_tick)
        .await
        .expect("Failed to set tick");

    let agent_id = Uuid::now_v7();
    df.add_alive_agent(agent_id)
        .await
        .expect("Failed to add agent");

    #[derive(Debug, Serialize, Deserialize)]
    struct SimpleState {
        energy: u32,
    }

    df.set_agent_state(agent_id, &SimpleState { energy: 75 })
        .await
        .expect("Failed to set agent state");

    // Phase 2: Flush events to PostgreSQL
    let event_store = EventStore::new(pg);
    let world_ctx = WorldContext {
        tick: test_tick,
        era: emergence_types::Era::Primitive,
        season: emergence_types::Season::Spring,
        weather: emergence_types::Weather::Clear,
        population: 1,
    };

    event_store
        .batch_insert(&[Event {
            id: EventId::new(),
            tick: test_tick,
            event_type: EventType::TickEnd,
            agent_id: None,
            location_id: None,
            details: serde_json::json!({}),
            agent_state_snapshot: None,
            world_context: world_ctx,
            created_at: Utc::now(),
        }])
        .await
        .expect("Failed to insert events");

    // Phase 3: Flush ledger entries
    let ledger_store = LedgerStore::new(pg);
    ledger_store
        .batch_insert(&[LedgerEntry {
            id: LedgerEntryId::new(),
            tick: test_tick,
            entry_type: LedgerEntryType::Regeneration,
            from_entity: None,
            from_entity_type: Some(EntityType::World),
            to_entity: Some(Uuid::now_v7()),
            to_entity_type: Some(EntityType::Location),
            resource: Resource::Water,
            quantity: Decimal::new(10, 0),
            reason: "REGEN".to_owned(),
            reference_id: None,
            created_at: Utc::now(),
        }])
        .await
        .expect("Failed to insert ledger entries");

    // Phase 4: Write world snapshot
    let snapshot_store = SnapshotStore::new(pg);
    snapshot_store
        .insert_world_snapshot(
            test_tick,
            "primitive",
            "spring",
            "clear",
            1,
            0,
            0,
            &serde_json::json!({"water": 1000}),
            &serde_json::json!({}),
            0,
            0,
            &serde_json::json!({"narrative": "test tick"}),
        )
        .await
        .expect("Failed to insert world snapshot");

    // Phase 5: Advance tick
    df.set_world_tick(test_tick + 1)
        .await
        .expect("Failed to advance tick");

    // Verify everything is consistent
    let current_tick = df
        .get_world_tick()
        .await
        .expect("Failed to get tick");
    assert_eq!(current_tick, test_tick + 1);

    let events = event_store
        .get_events_by_tick(test_tick)
        .await
        .expect("Failed to query events");
    assert_eq!(events.len(), 1);

    let ledger = ledger_store
        .get_entries_by_tick(test_tick)
        .await
        .expect("Failed to query ledger");
    assert_eq!(ledger.len(), 1);

    let snapshot = snapshot_store
        .get_world_snapshot(test_tick)
        .await
        .expect("Failed to query snapshot");
    assert!(snapshot.is_some());

    // Clean up
    sqlx::query("DELETE FROM events WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean events");
    sqlx::query("DELETE FROM ledger WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean ledger");
    sqlx::query("DELETE FROM world_snapshots WHERE tick = $1")
        .bind(i64::try_from(test_tick).unwrap_or(i64::MAX))
        .execute(pg)
        .await
        .expect("Failed to clean snapshots");
    df.flush_all().await.expect("Failed to flush Dragonfly");

    pg_pool.close().await;
}
