//! Tick callback that updates the Observer API state.
//!
//! After each tick, this callback updates the in-memory
//! [`SimulationSnapshot`] and broadcasts a [`TickBroadcast`] to all
//! connected WebSocket clients.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::Utc;
use emergence_core::runner::TickCallback;
use emergence_core::tick::{SimulationState, TickSummary};
use emergence_observer::state::{AppState, TickBroadcast, MAX_EVENTS};
use emergence_types::{
    AgentStateSnapshot, EconomyStats, Event, EventId, EventType, PopulationStats, WorldContext,
    WorldSnapshot,
};
use rust_decimal::Decimal;
use tracing::debug;

/// Callback that bridges the tick cycle to the Observer API.
pub struct ObserverCallback {
    state: Arc<AppState>,
}

impl ObserverCallback {
    /// Create a new observer callback backed by the given app state.
    pub const fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl TickCallback for ObserverCallback {
    #[allow(clippy::too_many_lines)]
    fn on_tick(&mut self, summary: &TickSummary, sim: &SimulationState) {
        // Build the broadcast message.
        let broadcast = TickBroadcast {
            tick: summary.tick,
            season: summary.season,
            weather: summary.weather,
            agents_alive: summary.agents_alive,
            #[allow(clippy::cast_possible_truncation)]
            deaths_this_tick: summary.deaths.len() as u32,
            #[allow(clippy::cast_possible_truncation)]
            actions_resolved: summary.action_results.len() as u32,
        };

        // Broadcast to WebSocket clients.
        let receivers = self.state.broadcast(&broadcast);
        debug!(tick = summary.tick, receivers, "Tick broadcast sent");

        // Update the snapshot. Use try_write to avoid blocking the tick
        // loop — if a REST handler holds the read lock, skip this update;
        // the next tick will catch up.
        if let Ok(mut snap) = self.state.snapshot.try_write() {
            // Basic fields
            snap.current_tick = summary.tick;
            snap.season = summary.season;
            snap.weather = summary.weather;
            snap.era = sim.clock.era();

            // Agent identity records
            snap.agents = sim.agents.clone();

            // Agent mutable state
            snap.agent_states = sim.agent_states.clone();

            // Locations from world map
            snap.locations = sim
                .world_map
                .locations()
                .map(|(id, loc_state)| (*id, loc_state.location.clone()))
                .collect();

            // Routes from world map
            snap.routes = sim
                .world_map
                .routes()
                .map(|(id, route)| (*id, route.clone()))
                .collect();

            // Build reusable world context for events
            let world_ctx = WorldContext {
                tick: summary.tick,
                era: sim.clock.era(),
                season: summary.season,
                weather: summary.weather,
                population: summary.agents_alive,
            };

            let mut new_events = Vec::new();

            // Death events
            for death in &summary.deaths {
                new_events.push(Event {
                    id: EventId::new(),
                    tick: summary.tick,
                    event_type: EventType::AgentDied,
                    agent_id: Some(death.agent_id),
                    location_id: Some(death.death_location),
                    details: serde_json::json!({
                        "cause": format!("{:?}", death.cause),
                        "final_age": death.final_age,
                    }),
                    agent_state_snapshot: None,
                    world_context: world_ctx.clone(),
                    created_at: Utc::now(),
                });
            }

            // Action events
            for (agent_id, result) in &summary.action_results {
                let event_type = if result.success {
                    EventType::ActionSucceeded
                } else {
                    EventType::ActionRejected
                };
                let agent_snap =
                    sim.agent_states
                        .get(agent_id)
                        .map(|s| AgentStateSnapshot {
                            energy: s.energy,
                            health: s.health,
                            hunger: s.hunger,
                            age: s.age,
                            location_id: s.location_id,
                            inventory_summary: s.inventory.clone(),
                        });
                new_events.push(Event {
                    id: EventId::new(),
                    tick: summary.tick,
                    event_type,
                    agent_id: Some(*agent_id),
                    location_id: sim.agent_states.get(agent_id).map(|s| s.location_id),
                    details: serde_json::json!({
                        "action_type": format!("{:?}", result.action_type),
                        "success": result.success,
                        "side_effects": result.side_effects,
                        "reason": result.rejection.as_ref().map(|r| format!("{:?}", r.reason)),
                        "message": result.rejection.as_ref().map(|r| &r.message),
                    }),
                    agent_state_snapshot: agent_snap,
                    world_context: world_ctx.clone(),
                    created_at: Utc::now(),
                });
            }

            // Append new events and cap at MAX_EVENTS
            snap.events.extend(new_events);
            if snap.events.len() > MAX_EVENTS {
                let drain_count = snap.events.len().saturating_sub(MAX_EVENTS);
                snap.events.drain(..drain_count);
            }

            // --- Compute WorldSnapshot ---

            #[allow(clippy::cast_possible_truncation)]
            let total_dead = sim
                .agents
                .values()
                .filter(|a| a.died_at_tick.is_some())
                .count() as u32;

            let alive_states: Vec<_> = sim
                .alive_agents
                .iter()
                .filter_map(|id| sim.agent_states.get(id))
                .collect();

            #[allow(clippy::arithmetic_side_effects)]
            let average_age = if alive_states.is_empty() {
                Decimal::ZERO
            } else {
                let total_age: u32 = alive_states.iter().map(|s| s.age).sum();
                #[allow(clippy::cast_possible_truncation)]
                let count = alive_states.len() as u32;
                Decimal::from(total_age) / Decimal::from(count)
            };

            let oldest_agent = alive_states
                .iter()
                .max_by_key(|s| s.age)
                .map(|s| s.agent_id);

            let population = PopulationStats {
                total_alive: summary.agents_alive,
                total_dead,
                births_this_tick: 0,
                #[allow(clippy::cast_possible_truncation)]
                deaths_this_tick: summary.deaths.len() as u32,
                average_age,
                oldest_agent,
            };

            // Economy: sum resources across agents and locations
            let mut resources_in_circulation = BTreeMap::new();
            for state in sim.agent_states.values() {
                for (&res, &qty) in &state.inventory {
                    let entry = resources_in_circulation.entry(res).or_insert(0u32);
                    *entry = entry.saturating_add(qty);
                }
            }

            let mut resources_at_nodes = BTreeMap::new();
            for (_, loc_state) in sim.world_map.locations() {
                for node in loc_state.location.base_resources.values() {
                    let entry = resources_at_nodes.entry(node.resource).or_insert(0u32);
                    *entry = entry.saturating_add(node.available);
                }
            }

            let mut total_resources = resources_in_circulation.clone();
            for (&res, &qty) in &resources_at_nodes {
                let entry = total_resources.entry(res).or_insert(0u32);
                *entry = entry.saturating_add(qty);
            }

            let economy = EconomyStats {
                total_resources,
                resources_in_circulation,
                resources_at_nodes,
                trades_this_tick: 0,
                gini_coefficient: Decimal::ZERO,
            };

            // All agent knowledge as discoveries
            let discoveries: Vec<String> = sim
                .agent_states
                .values()
                .flat_map(|s| s.knowledge.iter().cloned())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect();

            snap.world_snapshot = Some(WorldSnapshot {
                tick: summary.tick,
                era: sim.clock.era(),
                season: summary.season,
                weather: summary.weather,
                population,
                economy,
                discoveries,
                summary: format!(
                    "Tick {} complete. {} agents alive.",
                    summary.tick, summary.agents_alive
                ),
            });
        }
    }
}
