//! Shared application state for the Observer API server.
//!
//! [`AppState`] holds the broadcast channel for tick summaries and
//! in-memory snapshots of the simulation state that the REST endpoints
//! serve. In production this would be backed by Dragonfly and
//! `PostgreSQL`; in Phase 2 the in-memory store is sufficient.

use std::collections::BTreeMap;
use std::sync::Arc;

use emergence_core::operator::OperatorState;
use emergence_types::{
    Agent, AgentId, AgentState, Era, Event, Location, LocationId, Season, Weather, WorldSnapshot,
};
use tokio::sync::{broadcast, RwLock};

/// Capacity of the broadcast channel for tick summaries.
///
/// If a subscriber falls behind by more than this many messages it will
/// receive a [`broadcast::error::RecvError::Lagged`] and skip to the
/// newest message.
const BROADCAST_CAPACITY: usize = 256;

/// JSON-serializable tick summary pushed over the `WebSocket`.
///
/// This is a lightweight projection of the core [`emergence_core::tick::TickSummary`]
/// that can be safely serialized without pulling in the full core crate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TickBroadcast {
    /// The tick number.
    pub tick: u64,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
    /// Number of living agents.
    pub agents_alive: u32,
    /// Number of deaths this tick.
    pub deaths_this_tick: u32,
    /// Number of actions resolved this tick.
    pub actions_resolved: u32,
}

/// In-memory snapshot of the simulation state served by REST endpoints.
///
/// Updated each tick by the engine. All reads are served from this
/// snapshot so the observer never blocks the tick cycle.
#[derive(Debug, Clone)]
pub struct SimulationSnapshot {
    /// Agent identity records keyed by agent ID.
    pub agents: BTreeMap<AgentId, Agent>,
    /// Agent mutable state keyed by agent ID.
    pub agent_states: BTreeMap<AgentId, AgentState>,
    /// Location definitions keyed by location ID.
    pub locations: BTreeMap<LocationId, Location>,
    /// Event log (most recent first, capped for memory).
    pub events: Vec<Event>,
    /// The latest world snapshot.
    pub world_snapshot: Option<WorldSnapshot>,
    /// Current tick number.
    pub current_tick: u64,
    /// Current era.
    pub era: Era,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
}

impl Default for SimulationSnapshot {
    fn default() -> Self {
        Self {
            agents: BTreeMap::new(),
            agent_states: BTreeMap::new(),
            locations: BTreeMap::new(),
            events: Vec::new(),
            world_snapshot: None,
            current_tick: 0,
            era: Era::Primitive,
            season: Season::Spring,
            weather: Weather::Clear,
        }
    }
}

/// Shared state for the Axum application.
///
/// Wrapped in [`Arc`] and injected via Axum's `State` extractor.
/// The broadcast sender is used to push tick summaries to all
/// connected `WebSocket` clients. The snapshot is a read-write
/// lock protecting the simulation state.
#[derive(Clone)]
pub struct AppState {
    /// Broadcast sender for tick summary messages.
    pub tx: broadcast::Sender<TickBroadcast>,
    /// The current simulation snapshot (updated each tick).
    pub snapshot: Arc<RwLock<SimulationSnapshot>>,
    /// Shared operator control state (present when the simulation is running).
    pub operator_state: Option<Arc<OperatorState>>,
}

impl AppState {
    /// Create a new application state with an empty snapshot.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            tx,
            snapshot: Arc::new(RwLock::new(SimulationSnapshot::default())),
            operator_state: None,
        }
    }

    /// Create a new application state with operator control state attached.
    pub fn with_operator(operator: Arc<OperatorState>) -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            tx,
            snapshot: Arc::new(RwLock::new(SimulationSnapshot::default())),
            operator_state: Some(operator),
        }
    }

    /// Subscribe to the tick broadcast channel.
    ///
    /// Returns a receiver that will yield [`TickBroadcast`] messages
    /// for every tick the engine publishes.
    pub fn subscribe(&self) -> broadcast::Receiver<TickBroadcast> {
        self.tx.subscribe()
    }

    /// Publish a tick summary to all connected clients.
    ///
    /// Returns the number of receivers that received the message.
    /// Returns 0 if no clients are connected (this is not an error).
    pub fn broadcast(&self, summary: &TickBroadcast) -> usize {
        // send returns Err only when there are zero receivers,
        // which is normal when no WebSocket clients are connected.
        self.tx.send(summary.clone()).unwrap_or(0)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
