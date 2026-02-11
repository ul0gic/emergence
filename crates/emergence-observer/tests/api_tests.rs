//! Integration tests for the Observer API endpoints.
//!
//! Tests use Axum's `Router` directly via `tower::ServiceExt` without
//! starting a TCP server. This validates handler logic and routing
//! without needing a live network connection.

#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use emergence_observer::router::build_router;
use emergence_observer::state::{AppState, TickBroadcast};
use emergence_types::{
    Agent, AgentId, AgentState, Era, Event, EventId, EventType, Location, LocationId, Resource,
    ResourceNode, Season, Weather, WorldContext,
};
use rust_decimal::Decimal;
use serde_json::Value;
use tower::ServiceExt;

async fn make_test_state() -> Arc<AppState> {
    let state = Arc::new(AppState::new());

    let agent_id = AgentId::new();
    let location_id = LocationId::new();

    let agent = Agent {
        id: agent_id,
        name: String::from("TestAgent"),
        sex: emergence_types::Sex::Male,
        born_at_tick: 0,
        died_at_tick: None,
        cause_of_death: None,
        parent_a: None,
        parent_b: None,
        generation: 0,
        personality: emergence_types::Personality {
            curiosity: Decimal::new(5, 1),
            cooperation: Decimal::new(5, 1),
            aggression: Decimal::new(3, 1),
            risk_tolerance: Decimal::new(5, 1),
            industriousness: Decimal::new(7, 1),
            sociability: Decimal::new(4, 1),
            honesty: Decimal::new(8, 1),
            loyalty: Decimal::new(6, 1),
        },
        created_at: Utc::now(),
    };

    let agent_state = AgentState {
        agent_id,
        energy: 80,
        health: 100,
        hunger: 10,
        thirst: 0,
        age: 5,
        born_at_tick: 0,
        location_id,
        destination_id: None,
        travel_progress: 0,
        inventory: BTreeMap::new(),
        carry_capacity: 50,
        knowledge: BTreeSet::new(),
        skills: BTreeMap::new(),
        skill_xp: BTreeMap::new(),
        goals: Vec::new(),
        relationships: BTreeMap::new(),
        memory: Vec::new(),
    };

    let mut base_resources = BTreeMap::new();
    base_resources.insert(
        Resource::Wood,
        ResourceNode {
            resource: Resource::Wood,
            available: 50,
            regen_per_tick: 5,
            max_capacity: 100,
        },
    );

    let location = Location {
        id: location_id,
        name: String::from("Meadow"),
        region: String::from("Central"),
        location_type: String::from("natural"),
        description: String::from("A grassy meadow."),
        capacity: 20,
        base_resources,
        discovered_by: BTreeSet::new(),
        created_at: Utc::now(),
    };

    let event = Event {
        id: EventId::new(),
        tick: 1,
        event_type: EventType::TickStart,
        agent_id: None,
        location_id: None,
        details: serde_json::json!({}),
        agent_state_snapshot: None,
        world_context: WorldContext {
            tick: 1,
            era: Era::Primitive,
            season: Season::Spring,
            weather: Weather::Clear,
            population: 1,
        },
        created_at: Utc::now(),
    };

    // Populate snapshot
    {
        let mut snap = state.snapshot.write().await;
        snap.agents.insert(agent_id, agent);
        snap.agent_states.insert(agent_id, agent_state);
        snap.locations.insert(location_id, location);
        snap.events.push(event);
        snap.current_tick = 1;
        snap.era = Era::Primitive;
        snap.season = Season::Spring;
        snap.weather = Weather::Clear;
    }

    state
}

async fn body_to_json(body: Body) -> Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// =========================================================================
// Tests
// =========================================================================

#[tokio::test]
async fn test_index_returns_html() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_get_world() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(Request::get("/api/world").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["tick"], 1);
}

#[tokio::test]
async fn test_list_agents() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 1);
    assert_eq!(json["agents"][0]["name"], "TestAgent");
}

#[tokio::test]
async fn test_list_agents_filter_alive() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/agents?status=alive")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 1);
}

#[tokio::test]
async fn test_list_agents_filter_dead_returns_empty() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/agents?status=dead")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 0);
}

#[tokio::test]
async fn test_get_agent_by_id() {
    let state = make_test_state().await;

    let agent_id = {
        let snap = state.snapshot.read().await;
        *snap.agents.keys().next().unwrap()
    };

    let router = build_router(state);
    let path = format!("/api/agents/{}", agent_id.into_inner());
    let response = router
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["agent"]["name"], "TestAgent");
    assert!(json["state"].is_object());
}

#[tokio::test]
async fn test_get_agent_not_found() {
    let state = make_test_state().await;
    let router = build_router(state);

    let fake_id = uuid::Uuid::now_v7();
    let path = format!("/api/agents/{fake_id}");
    let response = router
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_agent_invalid_uuid() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/agents/not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_locations() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/locations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 1);
    assert_eq!(json["locations"][0]["name"], "Meadow");
}

#[tokio::test]
async fn test_get_location_by_id() {
    let state = make_test_state().await;

    let location_id = {
        let snap = state.snapshot.read().await;
        *snap.locations.keys().next().unwrap()
    };

    let router = build_router(state);
    let path = format!("/api/locations/{}", location_id.into_inner());
    let response = router
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["location"]["name"], "Meadow");
    assert!(json["agents_here"].is_array());
}

#[tokio::test]
async fn test_list_events() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 1);
}

#[tokio::test]
async fn test_list_events_filter_by_tick() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/events?tick=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 1);
}

#[tokio::test]
async fn test_list_events_filter_by_tick_no_match() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/events?tick=999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response.into_body()).await;
    assert_eq!(json["count"], 0);
}

#[tokio::test]
async fn test_broadcast_channel() {
    let state = AppState::new();
    let mut rx = state.subscribe();

    let summary = TickBroadcast {
        tick: 42,
        season: Season::Summer,
        weather: Weather::Clear,
        agents_alive: 10,
        deaths_this_tick: 0,
        actions_resolved: 10,
    };

    let receivers = state.broadcast(&summary);
    assert_eq!(receivers, 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.tick, 42);
    assert_eq!(received.agents_alive, 10);
}

#[tokio::test]
async fn test_nonexistent_route_returns_404() {
    let state = make_test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::get("/api/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
