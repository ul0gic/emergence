//! Tick cycle: the 6-phase engine loop that drives the Emergence simulation.
//!
//! Per `world-engine.md` section 2, each tick runs through these phases:
//!
//! 1. **World Wake** -- advance clock, generate weather, regenerate resources,
//!    apply vital mechanics (hunger, aging, starvation), advance travelers,
//!    process deaths.
//!
//! 2. **Perception** -- assemble a [`Perception`] payload for each living agent
//!    from world state, applying fog of war and fuzzy resource quantities.
//!
//! 3. **Decision** -- present perceptions to the [`DecisionSource`] and collect
//!    action requests (with deadline enforcement).
//!
//! 4. **Resolution** -- validate each action through the 7-stage pipeline,
//!    resolve conflicts for contested resources, execute valid actions, and
//!    reject invalid ones.
//!
//! 5. **Persist** -- (stub) flush state changes and events. In production this
//!    writes to Dragonfly and `PostgreSQL`; in Phase 2 it is a no-op.
//!
//! 6. **Reflection** -- (stub) post-tick analysis. Reserved for future phases.
//!
//! The tick cycle is deterministic given the same initial state and decision
//! source outputs.
//!
//! [`Perception`]: emergence_types::Perception

use std::collections::BTreeMap;

use emergence_types::{
    ActionParameters, ActionRequest, ActionResult, ActionType, AgentId, AgentState,
    LocationId, Perception, RejectionDetails, RejectionReason, Resource, Season, Weather,
};
use tracing::{debug, info, warn};

use crate::clock::WorldClock;
use crate::decision::DecisionSource;
use crate::perception::{self, PerceptionContext};
use emergence_agents::actions::conflict::{self, ClaimOutcome, ConflictStrategy, GatherClaim};
use emergence_agents::actions::handlers::{self, ExecutionContext};
use emergence_agents::actions::validation::{self, ValidationContext};
use emergence_agents::config::VitalsConfig;
use emergence_agents::death::DeathConsequences;
use emergence_agents::vitals;
use emergence_world::WorldMap;

/// Errors that can occur during tick execution.
#[derive(Debug, thiserror::Error)]
pub enum TickError {
    /// A clock operation failed.
    #[error("clock error: {source}")]
    Clock {
        /// The underlying clock error.
        #[from]
        source: crate::clock::ClockError,
    },

    /// An agent vital computation failed.
    #[error("agent error for {agent_id}: {source}")]
    Agent {
        /// The agent that caused the error.
        agent_id: AgentId,
        /// The underlying agent error.
        source: emergence_agents::AgentError,
    },

    /// A world operation failed.
    #[error("world error: {source}")]
    World {
        /// The underlying world error.
        #[from]
        source: emergence_world::WorldError,
    },

    /// The decision source failed.
    #[error("decision error: {source}")]
    Decision {
        /// The underlying decision error.
        #[from]
        source: crate::decision::DecisionError,
    },
}

/// Summary of a single tick's execution.
#[derive(Debug, Clone)]
pub struct TickSummary {
    /// The tick number that was executed.
    pub tick: u64,
    /// The season during this tick.
    pub season: Season,
    /// The weather during this tick.
    pub weather: Weather,
    /// Number of living agents at end of tick.
    pub agents_alive: u32,
    /// Agents who died during this tick.
    pub deaths: Vec<DeathConsequences>,
    /// Action results for each agent.
    pub action_results: BTreeMap<AgentId, ActionResult>,
    /// Resources regenerated at each location.
    pub regeneration: BTreeMap<LocationId, BTreeMap<Resource, u32>>,
}

/// Result of the World Wake phase.
struct WakeResult {
    /// Current season.
    season: Season,
    /// Current weather.
    weather: Weather,
    /// Resources regenerated at each location.
    regeneration: BTreeMap<LocationId, BTreeMap<Resource, u32>>,
    /// Agents who died this tick.
    deaths: Vec<DeathConsequences>,
}

/// Categorized actions after validation, split into gather claims (which
/// need conflict resolution) and non-gather actions (executed directly).
struct CategorizedActions {
    /// Gather claims grouped by (location, resource) for conflict resolution.
    gather_claims: BTreeMap<(LocationId, Resource), Vec<(AgentId, GatherClaim)>>,
    /// Non-gather actions to execute sequentially.
    non_gather: Vec<(AgentId, ActionRequest)>,
}

/// The mutable simulation state passed through the tick cycle.
///
/// This bundles all the state the engine needs to run a tick. In production,
/// this state is backed by Dragonfly; in tests it is held in memory.
#[derive(Debug)]
pub struct SimulationState {
    /// The world clock.
    pub clock: WorldClock,
    /// The world map (locations, routes, occupants).
    pub world_map: WorldMap,
    /// The weather system.
    pub weather_system: emergence_world::WeatherSystem,
    /// Agent identity data: agent\_id -> name (immutable after creation).
    pub agent_names: BTreeMap<AgentId, String>,
    /// Agent mutable state: agent\_id -> state.
    pub agent_states: BTreeMap<AgentId, AgentState>,
    /// Set of agent IDs that are alive.
    pub alive_agents: Vec<AgentId>,
    /// Vitals configuration.
    pub vitals_config: VitalsConfig,
    /// Conflict resolution strategy.
    pub conflict_strategy: ConflictStrategy,
}

/// Execute one complete tick of the simulation.
///
/// This is the main entry point for the engine. It runs all 6 phases
/// in sequence and returns a summary of what happened.
///
/// # Phases
///
/// 1. World Wake
/// 2. Perception
/// 3. Decision (via the provided `DecisionSource`)
/// 4. Resolution
/// 5. Persist (stub)
/// 6. Reflection (stub)
pub fn run_tick(
    state: &mut SimulationState,
    decision_source: &mut dyn DecisionSource,
) -> Result<TickSummary, TickError> {
    // --- Phase 1: World Wake ---
    let wake = phase_world_wake(state)?;

    let tick = state.clock.tick();
    info!(tick, season = ?wake.season, weather = ?wake.weather, "Tick started");

    // Remove dead agents from the alive list
    for death in &wake.deaths {
        state.alive_agents.retain(|id| *id != death.agent_id);
    }

    // --- Phase 2: Perception ---
    let perceptions = phase_perception(state, wake.season, wake.weather);

    // --- Phase 3: Decision ---
    let decisions = decision_source.collect_decisions(tick, &perceptions)?;

    // --- Phase 4: Resolution ---
    let action_results = phase_resolution(state, &decisions, wake.weather);

    // --- Phase 5: Persist (stub) ---
    debug!(tick, "Persist phase (stub)");

    // --- Phase 6: Reflection (stub) ---
    debug!(tick, "Reflection phase (stub)");

    let agents_alive = u32::try_from(state.alive_agents.len()).unwrap_or(u32::MAX);

    Ok(TickSummary {
        tick,
        season: wake.season,
        weather: wake.weather,
        agents_alive,
        deaths: wake.deaths,
        action_results,
        regeneration: wake.regeneration,
    })
}

/// Phase 1: World Wake.
///
/// Advances the clock, generates weather, regenerates resources, applies
/// vital mechanics to all agents, advances travelers, and processes deaths.
fn phase_world_wake(state: &mut SimulationState) -> Result<WakeResult, TickError> {
    // 1a. Advance clock
    state.clock.advance()?;
    let tick = state.clock.tick();

    // 1b. Derive season and generate weather
    let season = state.clock.season()?;
    let weather = state.weather_system.generate(tick, season);

    // 1c. Regenerate resources at all locations
    let regeneration = state.world_map.regenerate_all_resources(season)?;

    // 1d. Advance travelers and apply vitals
    let mut deaths = Vec::new();
    let agent_ids: Vec<AgentId> = state.alive_agents.clone();

    for agent_id in &agent_ids {
        let Some(agent_state) = state.agent_states.get_mut(agent_id) else {
            continue;
        };

        // Advance travel progress for traveling agents
        if agent_state.travel_progress > 0 {
            let arrived = handlers::advance_travel(agent_state).map_err(|source| {
                TickError::Agent {
                    agent_id: *agent_id,
                    source,
                }
            })?;
            if arrived {
                debug!(
                    tick,
                    ?agent_id,
                    location = ?agent_state.location_id,
                    "Agent arrived at destination"
                );
            }
        }

        // For Phase 2 simplicity, no agents are sheltered unless structures exist
        let is_sheltered = false;

        // Apply vital mechanics
        let vital_result =
            vitals::apply_vital_tick(agent_state, &state.vitals_config, is_sheltered).map_err(
                |source| TickError::Agent {
                    agent_id: *agent_id,
                    source,
                },
            )?;

        // Check for death
        if let Some(cause) = vital_result.death {
            let consequences =
                emergence_agents::death::process_death(agent_state, cause, Vec::new());
            info!(
                tick,
                agent_id = %consequences.agent_id,
                cause = %consequences.cause,
                age = consequences.final_age,
                "Agent died"
            );
            deaths.push(consequences);
        }
    }

    Ok(WakeResult {
        season,
        weather,
        regeneration,
        deaths,
    })
}

/// Phase 2: Perception.
///
/// Assembles a `Perception` payload for each living agent from world state.
fn phase_perception(
    state: &SimulationState,
    season: Season,
    weather: Weather,
) -> BTreeMap<AgentId, Perception> {
    let tick = state.clock.tick();
    let time_of_day = state.clock.time_of_day();
    let ticks_until_season_change = state.clock.ticks_until_season_change();

    let mut perceptions = BTreeMap::new();
    let mut location_contexts: BTreeMap<LocationId, PerceptionContext> = BTreeMap::new();

    for &agent_id in &state.alive_agents {
        let Some(agent_state) = state.agent_states.get(&agent_id) else {
            continue;
        };

        let location_id = agent_state.location_id;

        let ctx = location_contexts.entry(location_id).or_insert_with(|| {
            build_location_context(
                state,
                location_id,
                tick,
                time_of_day,
                season,
                weather,
                ticks_until_season_change,
            )
        });

        let agent_name = state
            .agent_names
            .get(&agent_id)
            .map_or("Unknown", String::as_str);

        let p = perception::assemble_perception(agent_state, agent_name, ctx);
        perceptions.insert(agent_id, p);
    }

    perceptions
}

/// Build a `PerceptionContext` for a specific location.
fn build_location_context(
    state: &SimulationState,
    location_id: LocationId,
    tick: u64,
    time_of_day: emergence_types::TimeOfDay,
    season: Season,
    weather: Weather,
    ticks_until_season_change: u64,
) -> PerceptionContext {
    let location_state = state.world_map.get_location(location_id);

    let (location_name, location_description, location_resources) =
        location_state.map_or_else(
            || {
                (
                    String::from("Unknown"),
                    String::from("An unknown place."),
                    BTreeMap::new(),
                )
            },
            |loc| {
                (
                    loc.location.name.clone(),
                    loc.location.description.clone(),
                    loc.available_resources(),
                )
            },
        );

    // Build agent names map for agents at this location
    let mut agent_names = BTreeMap::new();
    if let Some(loc) = location_state {
        for &occupant in &loc.occupants {
            if let Some(name) = state.agent_names.get(&occupant) {
                agent_names.insert(occupant, name.clone());
            }
        }
    }

    // Build known routes from this location
    let known_routes: Vec<emergence_types::KnownRoute> = state
        .world_map
        .neighbors(location_id)
        .iter()
        .filter_map(|(dest_id, _route_id)| {
            let dest_loc = state.world_map.get_location(*dest_id)?;
            let routes = state.world_map.routes_between(location_id, *dest_id);
            let first_route = routes.first()?;
            let cost_str = format!("{} ticks", first_route.cost_ticks);
            let path_str = format!("{:?}", first_route.path_type);
            Some(emergence_types::KnownRoute {
                destination: dest_loc.location.name.clone(),
                cost: cost_str,
                path_type: path_str,
            })
        })
        .collect();

    PerceptionContext {
        tick,
        time_of_day,
        season,
        weather,
        location_name,
        location_description,
        location_resources,
        structures_here: Vec::new(),
        messages_here: Vec::new(),
        known_routes,
        agent_names,
        ticks_until_season_change,
        message_expiry_ticks: perception::DEFAULT_MESSAGE_EXPIRY_TICKS,
    }
}

/// Phase 4: Resolution.
///
/// Validates each action, resolves conflicts, executes valid actions,
/// and returns results.
#[allow(clippy::too_many_lines)]
fn phase_resolution(
    state: &mut SimulationState,
    decisions: &BTreeMap<AgentId, ActionRequest>,
    weather: Weather,
) -> BTreeMap<AgentId, ActionResult> {
    let tick = state.clock.tick();
    let mut results = BTreeMap::new();

    // Categorize actions for conflict resolution
    let categorized = categorize_and_validate(state, decisions, weather, tick, &mut results);

    // Resolve gather conflicts and execute
    resolve_and_execute_gathers(state, &categorized.gather_claims, tick, &mut results);

    // Execute non-gather actions sequentially
    execute_non_gather_actions(state, &categorized.non_gather, weather, tick, &mut results);

    results
}

/// Categorize validated actions into gather claims (for conflict resolution)
/// and non-gather actions. Rejected actions are inserted directly into results.
fn categorize_and_validate(
    state: &SimulationState,
    decisions: &BTreeMap<AgentId, ActionRequest>,
    weather: Weather,
    tick: u64,
    results: &mut BTreeMap<AgentId, ActionResult>,
) -> CategorizedActions {
    let mut gather_claims: BTreeMap<(LocationId, Resource), Vec<(AgentId, GatherClaim)>> =
        BTreeMap::new();
    let mut non_gather_actions: Vec<(AgentId, ActionRequest)> = Vec::new();

    for (&agent_id, request) in decisions {
        if !state.alive_agents.contains(&agent_id) {
            continue;
        }

        let Some(agent_state) = state.agent_states.get(&agent_id) else {
            continue;
        };

        let location_id = agent_state.location_id;
        let is_traveling = agent_state.destination_id.is_some();
        let location_resources = state
            .world_map
            .get_location(location_id)
            .map(|loc| loc.resources().clone())
            .unwrap_or_default();
        let agents_at_location: Vec<AgentId> = state
            .world_map
            .get_location(location_id)
            .map(|loc| loc.occupants.iter().copied().collect())
            .unwrap_or_default();
        let travel_blocked = weather == Weather::Storm;

        // An agent is mature if they have lived at least `maturity_ticks` since birth.
        // Seed agents (born_at_tick = 0) become mature after maturity_ticks elapse.
        // The maturity check uses the tick-based age: current_tick - born_at_tick.
        let maturity_ticks = emergence_agents::default_maturity_ticks();
        let is_mature = emergence_agents::is_mature(agent_state.born_at_tick, tick, maturity_ticks);

        // Look up the route for Move actions (needed for ACL and toll checks).
        let move_route = if let ActionParameters::Move { destination } = &request.parameters {
            state.world_map.find_route_from_to(location_id, *destination).cloned()
        } else {
            None
        };

        let validation_ctx = ValidationContext {
            agent_id,
            agent_location: location_id,
            is_traveling,
            location_resources,
            agents_at_location,
            travel_blocked,
            agent_knowledge: agent_state.knowledge.clone(),
            is_mature,
            structures_at_location: std::collections::BTreeMap::new(),
            route_to_improve: None,
            move_route,
            agent_groups: Vec::new(), // TODO: populate from social graph when available
            dead_agents: std::collections::BTreeSet::new(), // TODO: populate from agent manager
            farm_registry: emergence_world::FarmRegistry::new(), // TODO: populate from world state
            library_knowledge: std::collections::BTreeMap::new(), // TODO: populate from library state
            current_tick: tick,
        };

        let validation_result = validation::validate_action(
            request.action_type,
            &request.parameters,
            agent_state,
            &validation_ctx,
        );

        if let Err(reason) = validation_result {
            debug!(tick, ?agent_id, action = ?request.action_type, ?reason, "Action rejected");
            results.insert(agent_id, make_rejection(tick, agent_id, request.action_type, reason));
            continue;
        }

        if let (ActionType::Gather, ActionParameters::Gather { resource }) =
            (request.action_type, &request.parameters)
        {
            let claim = GatherClaim {
                agent_id,
                resource: *resource,
                requested: emergence_agents::actions::costs::BASE_GATHER_YIELD,
                submitted_at: request.submitted_at,
            };
            gather_claims
                .entry((location_id, *resource))
                .or_default()
                .push((agent_id, claim));
        } else {
            non_gather_actions.push((agent_id, request.clone()));
        }
    }

    CategorizedActions {
        gather_claims,
        non_gather: non_gather_actions,
    }
}

/// Resolve gather conflicts and execute the granted gathers.
fn resolve_and_execute_gathers(
    state: &mut SimulationState,
    gather_claims: &BTreeMap<(LocationId, Resource), Vec<(AgentId, GatherClaim)>>,
    tick: u64,
    results: &mut BTreeMap<AgentId, ActionResult>,
) {
    for ((location_id, resource), claims_group) in gather_claims {
        let available = state
            .world_map
            .get_location(*location_id)
            .and_then(|loc| loc.get_resource(resource).map(|n| n.available))
            .unwrap_or(0);

        let claims: Vec<GatherClaim> = claims_group.iter().map(|(_, c)| c.clone()).collect();
        let outcomes =
            conflict::resolve_gather_conflict(available, &claims, state.conflict_strategy);

        for (agent_id, _claim) in claims_group {
            match outcomes.get(agent_id) {
                Some(ClaimOutcome::Granted { quantity }) if *quantity > 0 => {
                    execute_single_gather(state, *agent_id, *location_id, *resource, tick, results);
                }
                Some(ClaimOutcome::Rejected { reason }) => {
                    results.insert(*agent_id, make_rejection(tick, *agent_id, ActionType::Gather, *reason));
                }
                _ => {
                    results.insert(*agent_id, make_rejection(tick, *agent_id, ActionType::Gather, RejectionReason::ConflictLost));
                }
            }
        }
    }
}

/// Execute a single gather action for an agent.
///
/// Pre-computes immutable reads from `state` before taking the mutable
/// borrow on the agent state to satisfy the borrow checker.
fn execute_single_gather(
    state: &mut SimulationState,
    agent_id: AgentId,
    location_id: LocationId,
    resource: Resource,
    tick: u64,
    results: &mut BTreeMap<AgentId, ActionResult>,
) {
    // Pre-compute immutable reads before mutable borrow on agent_states.
    let loc_resources = state
        .world_map
        .get_location(location_id)
        .map(emergence_world::LocationState::available_resources)
        .unwrap_or_default();
    let vitals_config = state.vitals_config.clone();

    let agent_name = state
        .agent_names
        .get(&agent_id)
        .cloned()
        .unwrap_or_default();

    let Some(agent_state) = state.agent_states.get_mut(&agent_id) else {
        return;
    };

    let mut exec_ctx = ExecutionContext {
        location_resources: loc_resources,
        is_sheltered: false,
        shelter_bonus_pct: 100,
        travel_cost: None,
        move_destination: None,
        current_tick: tick,
        agent_name,
        structures_at_location: std::collections::BTreeMap::new(),
        route_to_improve: None,
        move_toll_cost: None,
        dead_agents: std::collections::BTreeSet::new(),
        agent_groups: std::collections::BTreeSet::new(),
        active_rules: std::collections::BTreeMap::new(),
        farm_registry: emergence_world::FarmRegistry::new(),
        library_knowledge: std::collections::BTreeMap::new(),
    };

    match handlers::execute_gather(agent_state, resource, &vitals_config, &mut exec_ctx) {
        Ok(hr) => {
            // Drop the mutable borrow on agent_state before borrowing world_map.
            for (res, qty) in &hr.location_resource_deltas {
                if let Some(loc) = state.world_map.get_location_mut(location_id) {
                    let _ = loc.harvest_resource(*res, *qty);
                }
            }
            results.insert(
                agent_id,
                ActionResult {
                    tick,
                    agent_id,
                    action_type: ActionType::Gather,
                    success: true,
                    outcome: Some(hr.outcome),
                    rejection: None,
                    side_effects: Vec::new(),
                },
            );
        }
        Err(err) => {
            warn!(tick, ?agent_id, %err, "Gather execution failed");
            results.insert(agent_id, make_rejection(tick, agent_id, ActionType::Gather, RejectionReason::CapacityExceeded));
        }
    }
}

/// Execute non-gather actions sequentially.
///
/// To satisfy the borrow checker, we pre-compute all immutable reads from
/// `state` (location resources, travel cost, vitals config clone) before
/// taking the mutable borrow on the agent state.
fn execute_non_gather_actions(
    state: &mut SimulationState,
    non_gather_actions: &[(AgentId, ActionRequest)],
    weather: Weather,
    tick: u64,
    results: &mut BTreeMap<AgentId, ActionResult>,
) {
    // Pre-compute immutable data for each action before any mutable borrows.
    let precomputed: Vec<_> = non_gather_actions
        .iter()
        .filter_map(|(agent_id, request)| {
            let agent_state = state.agent_states.get(agent_id)?;
            let location_id = agent_state.location_id;
            let loc_resources = state
                .world_map
                .get_location(location_id)
                .map(emergence_world::LocationState::available_resources)
                .unwrap_or_default();
            let travel_cost =
                compute_travel_cost_from_map(&state.world_map, location_id, &request.parameters, weather);
            let move_destination = extract_move_destination(&request.parameters);
            let move_toll_cost =
                extract_move_toll_cost(&state.world_map, location_id, &request.parameters);
            let agent_name = state
                .agent_names
                .get(agent_id)
                .cloned()
                .unwrap_or_default();
            Some((*agent_id, request.clone(), location_id, loc_resources, travel_cost, move_destination, move_toll_cost, agent_name))
        })
        .collect();

    // Clone vitals config once to avoid borrowing state during mutable agent access.
    let vitals_config = state.vitals_config.clone();

    for (agent_id, request, location_id, loc_resources, travel_cost, move_destination, move_toll_cost, agent_name) in &precomputed {
        let Some(agent_state) = state.agent_states.get_mut(agent_id) else {
            continue;
        };

        let mut exec_ctx = ExecutionContext {
            location_resources: loc_resources.clone(),
            is_sheltered: false,
            shelter_bonus_pct: 100,
            travel_cost: *travel_cost,
            move_destination: *move_destination,
            current_tick: tick,
            agent_name: agent_name.clone(),
            structures_at_location: std::collections::BTreeMap::new(),
            route_to_improve: None,
            move_toll_cost: move_toll_cost.clone(),
            dead_agents: std::collections::BTreeSet::new(),
            agent_groups: std::collections::BTreeSet::new(),
            active_rules: std::collections::BTreeMap::new(),
            farm_registry: emergence_world::FarmRegistry::new(),
            library_knowledge: std::collections::BTreeMap::new(),
        };

        match handlers::execute_action(
            request.action_type,
            &request.parameters,
            agent_state,
            &vitals_config,
            &mut exec_ctx,
        ) {
            Ok(hr) => {
                for (res, qty) in &hr.location_resource_deltas {
                    if let Some(loc) = state.world_map.get_location_mut(*location_id) {
                        let _ = loc.harvest_resource(*res, *qty);
                    }
                }
                results.insert(
                    *agent_id,
                    ActionResult {
                        tick,
                        agent_id: *agent_id,
                        action_type: request.action_type,
                        success: true,
                        outcome: Some(hr.outcome),
                        rejection: None,
                        side_effects: Vec::new(),
                    },
                );
            }
            Err(err) => {
                warn!(tick, ?agent_id, %err, "Action execution failed");
                results.insert(*agent_id, make_rejection(tick, *agent_id, request.action_type, RejectionReason::InvalidAction));
            }
        }
    }
}

/// Compute the travel cost for a move action, or `None` for non-move actions.
///
/// Takes `&WorldMap` directly to avoid borrow-checker conflicts when
/// `SimulationState` is partially borrowed.
fn compute_travel_cost_from_map(
    world_map: &emergence_world::WorldMap,
    from: LocationId,
    params: &ActionParameters,
    weather: Weather,
) -> Option<u32> {
    if let ActionParameters::Move { destination } = params {
        let routes = world_map.routes_between(from, *destination);
        routes.first().and_then(|r| {
            emergence_world::route::effective_travel_cost(r, weather)
                .ok()
                .flatten()
        })
    } else {
        None
    }
}

/// Extract the destination from move parameters, or `None`.
const fn extract_move_destination(params: &ActionParameters) -> Option<LocationId> {
    if let ActionParameters::Move { destination } = params {
        Some(*destination)
    } else {
        None
    }
}

/// Extract the toll cost for a move action, or `None` for non-move actions
/// or routes without a toll.
///
/// Takes `&WorldMap` directly to avoid borrow-checker conflicts when
/// `SimulationState` is partially borrowed.
fn extract_move_toll_cost(
    world_map: &emergence_world::WorldMap,
    from: LocationId,
    params: &ActionParameters,
) -> Option<std::collections::BTreeMap<Resource, u32>> {
    if let ActionParameters::Move { destination } = params {
        let routes = world_map.routes_between(from, *destination);
        routes.first().and_then(|r| {
            emergence_world::route::toll_cost(r).cloned()
        })
    } else {
        None
    }
}

/// Build a rejection `ActionResult`.
fn make_rejection(
    tick: u64,
    agent_id: AgentId,
    action_type: ActionType,
    reason: RejectionReason,
) -> ActionResult {
    ActionResult {
        tick,
        agent_id,
        action_type,
        success: false,
        outcome: None,
        rejection: Some(RejectionDetails {
            reason,
            message: format!("{reason:?}"),
        }),
        side_effects: Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use chrono::Utc;
    use emergence_types::*;
    use rust_decimal::Decimal;

    use super::*;
    use crate::clock::WorldClock;
    use crate::config::TimeConfig;
    use crate::decision::StubDecisionSource;

    fn default_time_config() -> TimeConfig {
        TimeConfig {
            ticks_per_season: 90,
            seasons: vec![
                "spring".to_owned(),
                "summer".to_owned(),
                "autumn".to_owned(),
                "winter".to_owned(),
            ],
            day_night: true,
        }
    }

    fn make_agent_state(agent_id: AgentId, location_id: LocationId) -> AgentState {
        AgentState {
            agent_id,
            energy: 80,
            health: 100,
            hunger: 0,
            age: 0,
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
        }
    }

    fn make_location(id: LocationId, name: &str) -> Location {
        let mut resources = BTreeMap::new();
        resources.insert(
            Resource::Wood,
            ResourceNode {
                resource: Resource::Wood,
                available: 50,
                regen_per_tick: 5,
                max_capacity: 100,
            },
        );
        resources.insert(
            Resource::Water,
            ResourceNode {
                resource: Resource::Water,
                available: 100,
                regen_per_tick: 10,
                max_capacity: 200,
            },
        );
        resources.insert(
            Resource::FoodBerry,
            ResourceNode {
                resource: Resource::FoodBerry,
                available: 30,
                regen_per_tick: 3,
                max_capacity: 50,
            },
        );
        Location {
            id,
            name: name.to_string(),
            region: String::from("Test"),
            location_type: String::from("natural"),
            description: format!("Test location: {name}"),
            capacity: 20,
            base_resources: resources,
            discovered_by: BTreeSet::new(),
            created_at: Utc::now(),
        }
    }

    fn make_route(from: LocationId, to: LocationId) -> Route {
        Route {
            id: RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: 3,
            path_type: PathType::DirtTrail,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: None,
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    fn make_simulation_state() -> SimulationState {
        let time_config = default_time_config();
        let clock = WorldClock::new(&time_config).unwrap();

        let mut world_map = WorldMap::new();
        let loc_a = LocationId::new();
        let loc_b = LocationId::new();

        let _ = world_map.add_location(make_location(loc_a, "Meadow"));
        let _ = world_map.add_location(make_location(loc_b, "Forest"));
        let _ = world_map.add_route(make_route(loc_a, loc_b));

        let agent_id = AgentId::new();
        let agent_state = make_agent_state(agent_id, loc_a);

        if let Some(loc) = world_map.get_location_mut(loc_a) {
            let _ = loc.add_occupant(agent_id);
        }

        let mut agent_names = BTreeMap::new();
        agent_names.insert(agent_id, String::from("Alpha"));

        let mut agent_states = BTreeMap::new();
        agent_states.insert(agent_id, agent_state);

        SimulationState {
            clock,
            world_map,
            weather_system: emergence_world::WeatherSystem::new(42),
            agent_names,
            agent_states,
            alive_agents: vec![agent_id],
            vitals_config: VitalsConfig::default(),
            conflict_strategy: ConflictStrategy::FirstComeFirstServed,
        }
    }

    #[test]
    fn tick_advances_clock() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let result = run_tick(&mut state, &mut decisions);
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.tick, 1);
    }

    #[test]
    fn tick_applies_hunger() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let agent_id = *state.alive_agents.first().unwrap();
        let initial_hunger = state.agent_states.get(&agent_id).unwrap().hunger;

        let _ = run_tick(&mut state, &mut decisions);

        let new_hunger = state.agent_states.get(&agent_id).unwrap().hunger;
        assert_eq!(new_hunger, initial_hunger + 5);
    }

    #[test]
    fn tick_regenerates_resources() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let result = run_tick(&mut state, &mut decisions);
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(!summary.regeneration.is_empty());
    }

    #[test]
    fn stub_decisions_produce_no_action_results() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let result = run_tick(&mut state, &mut decisions);
        assert!(result.is_ok());
        let summary = result.unwrap();

        for (_, action_result) in &summary.action_results {
            assert_eq!(action_result.action_type, ActionType::NoAction);
            assert!(action_result.success);
        }
    }

    #[test]
    fn multiple_ticks_run_without_error() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        for expected_tick in 1..=10 {
            let result = run_tick(&mut state, &mut decisions);
            assert!(result.is_ok());
            let summary = result.unwrap();
            assert_eq!(summary.tick, expected_tick);
        }
    }

    #[test]
    fn agent_dies_from_starvation_over_time() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let agent_id = *state.alive_agents.first().unwrap();

        let mut agent_died = false;
        for _ in 0..100 {
            let result = run_tick(&mut state, &mut decisions);
            assert!(result.is_ok());
            let summary = result.unwrap();

            if !summary.deaths.is_empty() {
                agent_died = true;
                assert!(summary.deaths.iter().any(|d| d.agent_id == agent_id));
                break;
            }
        }
        assert!(agent_died, "Agent should have died from starvation");
    }

    #[test]
    fn dead_agents_removed_from_alive_list() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let agent_id = *state.alive_agents.first().unwrap();

        if let Some(agent_state) = state.agent_states.get_mut(&agent_id) {
            agent_state.hunger = 96;
            agent_state.health = 5;
        }

        let _ = run_tick(&mut state, &mut decisions);
        assert!(!state.alive_agents.contains(&agent_id));
    }

    #[test]
    fn tick_summary_has_correct_agent_count() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();

        let result = run_tick(&mut state, &mut decisions);
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.agents_alive, 1);
    }
}
