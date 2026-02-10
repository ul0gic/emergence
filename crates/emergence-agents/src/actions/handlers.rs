//! Execution handlers for survival actions.
//!
//! Each handler assumes the action has already passed the validation pipeline
//! (stages 1--6). The handler executes the side effects: modifying agent state,
//! withdrawing from location resources, producing ledger-ready resource deltas.
//!
//! Per `world-engine.md` section 7.1, the survival actions are:
//! - Gather: collect resources from the current location
//! - Eat: consume food to reduce hunger and restore energy
//! - Drink: consume water for hydration
//! - Rest: recover energy (bonus if sheltered)
//! - Move: begin multi-tick travel to an adjacent location

use std::collections::{BTreeMap, BTreeSet};

use emergence_types::{
    ActionOutcome, ActionParameters, ActionType, AgentId, AgentState, EnforcementAppliedDetails,
    GroupId, LocationId, Message, PathType, Resource, Route, Rule, RuleId, Structure, StructureId,
    StructureType,
};

use emergence_world::farming;
use emergence_world::route as world_route;
use emergence_world::structure as world_structure;

use crate::config::VitalsConfig;
use crate::crafting;
use crate::error::AgentError;
use crate::inventory;
use crate::skills;
use crate::skills::effects;
use crate::vitals;

use super::costs;

/// Maximum allowed length for a message content string (characters).
const MAX_MESSAGE_LENGTH: usize = 500;

/// Context provided by the tick cycle for executing an action against world state.
///
/// The handler may read or mutate these values. The caller is responsible for
/// applying the resulting changes to the actual world map after the handler
/// returns.
#[derive(Debug)]
pub struct ExecutionContext {
    /// Resources available at the agent's current location.
    /// The handler will decrement available quantities when gathering.
    pub location_resources: BTreeMap<Resource, u32>,
    /// Whether the agent is currently sheltered (for rest bonus).
    pub is_sheltered: bool,
    /// Shelter rest bonus percentage (100 = no bonus, 150 = 1.5x, etc.).
    pub shelter_bonus_pct: u32,
    /// The route travel cost in ticks for a move action (set by caller
    /// after looking up the route). `None` if not a move action.
    pub travel_cost: Option<u32>,
    /// The destination location for a move action.
    pub move_destination: Option<LocationId>,
    /// The current tick number (needed for message timestamps).
    pub current_tick: u64,
    /// The agent's display name (needed for message sender name).
    pub agent_name: String,
    /// Structures at the agent's current location, keyed by structure ID.
    /// Populated for build, repair, and demolish actions.
    pub structures_at_location: BTreeMap<StructureId, Structure>,
    /// The route being targeted by an `ImproveRoute` action, if any.
    ///
    /// Populated by the tick cycle from the world map. Contains a clone of
    /// the route connecting the agent's location to the action's destination.
    /// The handler will determine whether to upgrade or repair.
    pub route_to_improve: Option<Route>,
    /// Toll cost for traversing the route in a `Move` action, if any.
    ///
    /// Populated by the tick cycle from the route's ACL. When present, the
    /// move handler deducts these resources from the agent's inventory as
    /// payment for passage. The toll is paid to the route builder (owner).
    pub move_toll_cost: Option<BTreeMap<Resource, u32>>,
    /// Set of agent IDs known to be dead.
    ///
    /// Used by the `Claim` handler to determine if a structure's owner has
    /// died, making it eligible for claiming. Populated by the tick cycle.
    pub dead_agents: BTreeSet<AgentId>,
    /// Groups the acting agent belongs to.
    ///
    /// Used by the `Legislate` and `Enforce` handlers to verify group
    /// membership. Populated by the tick cycle from the social graph.
    pub agent_groups: BTreeSet<GroupId>,
    /// Active rules in the simulation, keyed by [`RuleId`].
    ///
    /// Used by the `Enforce` handler to look up the rule being enforced
    /// and verify the enforcer has authority. Populated by the tick cycle.
    pub active_rules: BTreeMap<RuleId, Rule>,
    /// The farm registry tracking crop growth state on farm plots.
    ///
    /// Populated by the tick cycle. Used by `FarmPlant` and `FarmHarvest`
    /// handlers to manage planting and harvesting lifecycle.
    pub farm_registry: farming::FarmRegistry,
    /// Knowledge concepts stored in library structures at this location.
    ///
    /// Populated by the tick cycle from library state in Dragonfly.
    /// Key is the structure ID of the library, value is the set of concepts
    /// written to it. Used by `Write` and `Read` actions.
    pub library_knowledge: BTreeMap<StructureId, BTreeSet<String>>,
}

/// Result of executing an action handler, containing the changes to apply.
#[derive(Debug, Clone)]
pub struct HandlerResult {
    /// The `ActionOutcome` to return to the agent.
    pub outcome: ActionOutcome,
    /// Resources harvested from the location (resource -> quantity).
    /// The caller must decrement the location's resource nodes by these amounts.
    pub location_resource_deltas: BTreeMap<Resource, u32>,
    /// Whether the agent began traveling (move action).
    pub began_travel: bool,
    /// Messages produced by the action (communicate, broadcast).
    /// The caller must push these to the appropriate location message
    /// boards in Dragonfly.
    pub messages: Vec<Message>,
    /// Structure that was built this tick, if any.
    /// The caller must add this to the world map and location state.
    pub structure_built: Option<Structure>,
    /// Structure ID that was repaired this tick, if any.
    /// The caller must call `apply_repair` on the structure in world state.
    pub structure_repaired: Option<StructureId>,
    /// Structure ID that was demolished this tick, if any.
    /// The caller must remove this from the world map and location state.
    pub structure_demolished: Option<StructureId>,
    /// Route that was upgraded this tick, if any.
    ///
    /// Contains `(old_path_type, new_path_type, materials_used)`. The caller
    /// must apply the upgrade to the actual route in the world map via
    /// [`world_route::apply_route_upgrade`] and emit the `RouteImproved` event.
    pub route_upgraded: Option<(PathType, PathType, BTreeMap<Resource, u32>)>,
    /// Whether a route was repaired this tick (durability restored without
    /// changing path type). Contains the durability restored. The caller must
    /// apply the repair to the actual route in the world map via
    /// [`world_route::repair_route`] and emit the `RouteImproved` event
    /// with `is_repair = true`.
    pub route_repaired: Option<u32>,
    /// Structure ID whose ownership was changed by a `Claim` action.
    ///
    /// The caller must update the structure's `owner` field in world state
    /// and emit a `StructureClaimed` event.
    pub structure_claimed: Option<StructureId>,
    /// A governance rule created by a `Legislate` action.
    ///
    /// The caller must store this rule in the active rules registry and
    /// emit a `RuleCreated` event.
    pub rule_created: Option<Rule>,
    /// Enforcement details from an `Enforce` action.
    ///
    /// The caller must apply relationship penalties and emit an
    /// `EnforcementApplied` event.
    pub enforcement: Option<EnforcementAppliedDetails>,
    /// Farm plot that had crops planted this tick, if any.
    ///
    /// Contains `(structure_id, mature_at_tick)`. The caller must update
    /// the farm registry in world state.
    pub farm_planted: Option<(StructureId, u64)>,
    /// Farm plot that was harvested this tick, if any.
    ///
    /// Contains the structure ID. The caller must clear the crop state
    /// in the farm registry.
    pub farm_harvested: Option<StructureId>,
    /// Knowledge written to a library this tick, if any.
    ///
    /// Contains `(library_structure_id, concept)`. The caller must persist
    /// the concept to the library's knowledge set.
    pub library_write: Option<(StructureId, String)>,
    /// Knowledge read from a library this tick, if any.
    ///
    /// Contains `(library_structure_id, concept)`. The caller must add
    /// the concept to the agent's knowledge base via
    /// [`KnowledgeBase::learn`](crate::knowledge::KnowledgeBase::learn).
    pub library_read: Option<(StructureId, String)>,
}

/// Execute a gather action: collect resources from the agent's location.
///
/// The gather yield is `BASE_GATHER_YIELD + skill_level / 2` (via
/// [`effects::gathering_yield`]), where `skill_level` is the agent's
/// "gathering" skill. The actual amount taken is capped by what the
/// location has available.
///
/// Awards [`skills::XP_GATHER`] (10) gathering XP on success.
///
/// Modifies:
/// - Agent inventory (adds gathered resource)
/// - Agent energy (deducts gather cost)
/// - Agent skill XP (adds gathering XP)
pub fn execute_gather(
    agent: &mut AgentState,
    resource: Resource,
    _config: &VitalsConfig,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Compute yield using skill effects
    let skill_level = agent
        .skills
        .get("gathering")
        .copied()
        .unwrap_or(0);
    let target_yield = effects::gathering_yield(costs::BASE_GATHER_YIELD, skill_level)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("gather yield overflow"),
        })?;

    // Cap by what the location actually has
    let available = ctx
        .location_resources
        .get(&resource)
        .copied()
        .unwrap_or(0);
    let actual = target_yield.min(available);

    // Add to agent inventory
    inventory::add_resource(
        &mut agent.inventory,
        agent.carry_capacity,
        resource,
        actual,
    )?;

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Gather));

    // Update location resource tracking
    if let Some(loc_avail) = ctx.location_resources.get_mut(&resource) {
        *loc_avail = loc_avail.saturating_sub(actual);
    }

    // Award gathering XP
    let xp_gained = skills::XP_GATHER;
    let xp_entry = agent.skill_xp.entry(String::from("gathering")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("gathering XP overflow"),
        }
    })?;

    // Build resource changes for the outcome
    let mut resource_changes = BTreeMap::new();
    let actual_i64 = i64::from(actual);
    resource_changes.insert(resource, actual_i64);

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("gathering"), xp_gained);

    let mut location_deltas = BTreeMap::new();
    location_deltas.insert(resource, actual);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Gather),
            skill_xp,
            details: serde_json::json!({
                "resource": format!("{resource:?}"),
                "yield": actual,
                "skill_level": skill_level,
            }),
        },
        location_resource_deltas: location_deltas,
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute an eat action: consume food from inventory.
///
/// Reduces hunger and restores energy per the food values table in
/// `world-engine.md` section 6.2.
///
/// Modifies:
/// - Agent inventory (removes 1 unit of food)
/// - Agent hunger (decreased)
/// - Agent energy (increased)
pub fn execute_eat(
    agent: &mut AgentState,
    food_type: Resource,
    config: &VitalsConfig,
) -> Result<HandlerResult, AgentError> {
    // Look up food values
    let (hunger_reduction, energy_gain) =
        costs::food_values(food_type).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("non-food resource passed to execute_eat"),
        })?;

    // Remove 1 unit from inventory
    inventory::remove_resource(&mut agent.inventory, food_type, 1)?;

    // Apply eating effects
    vitals::apply_eat(agent, config, hunger_reduction, energy_gain)?;

    // Eat costs 0 energy (already 0 in costs table, but be explicit)
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Eat));

    let mut resource_changes = BTreeMap::new();
    resource_changes.insert(food_type, -1);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Eat),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "food_type": format!("{food_type:?}"),
                "hunger_reduction": hunger_reduction,
                "energy_gain": energy_gain,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a drink action: consume water from location or inventory.
///
/// Prefers location water first (free), then inventory water.
/// Reduces hunger by a small amount and restores energy.
///
/// Modifies:
/// - Location water (decremented if drinking from location)
/// - Agent inventory (decremented if drinking from inventory)
/// - Agent hunger (slightly reduced)
/// - Agent energy (slightly restored)
pub fn execute_drink(
    agent: &mut AgentState,
    config: &VitalsConfig,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let mut resource_changes = BTreeMap::new();
    let mut location_deltas = BTreeMap::new();

    // Drink from location first, then inventory
    let loc_water = ctx
        .location_resources
        .get(&Resource::Water)
        .copied()
        .unwrap_or(0);

    if loc_water > 0 {
        // Drink from location
        if let Some(w) = ctx.location_resources.get_mut(&Resource::Water) {
            *w = w.saturating_sub(1);
        }
        location_deltas.insert(Resource::Water, 1);
    } else {
        // Drink from inventory
        inventory::remove_resource(&mut agent.inventory, Resource::Water, 1)?;
        resource_changes.insert(Resource::Water, -1);
    }

    // Drinking provides minor hunger reduction (5) and energy gain (5)
    let hunger_reduction: u32 = 5;
    let energy_gain: u32 = 5;
    agent.hunger = agent.hunger.saturating_sub(hunger_reduction);
    agent.energy = agent.energy.checked_add(energy_gain).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("energy gain overflow in drink"),
        }
    })?;

    // Clamp energy to age-based max
    let max_energy = config
        .max_energy_for_age(agent.age)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("max_energy_for_age overflow in drink"),
        })?;
    if agent.energy > max_energy {
        agent.energy = max_energy;
    }

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Drink));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Drink),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "source": if location_deltas.is_empty() { "inventory" } else { "location" },
                "hunger_reduction": hunger_reduction,
                "energy_gain": energy_gain,
            }),
        },
        location_resource_deltas: location_deltas,
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a rest action: recover energy.
///
/// Rest recovery is modified by the shelter bonus if the agent is sheltered.
///
/// Modifies:
/// - Agent energy (increased by rest recovery, clamped to max)
pub fn execute_rest(
    agent: &mut AgentState,
    config: &VitalsConfig,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let bonus_pct = if ctx.is_sheltered {
        ctx.shelter_bonus_pct
    } else {
        100
    };

    let energy_before = agent.energy;

    vitals::apply_rest(agent, config, bonus_pct)?;
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Rest));

    let energy_recovered = agent.energy.saturating_sub(energy_before);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Rest),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "energy_recovered": energy_recovered,
                "sheltered": ctx.is_sheltered,
                "shelter_bonus_pct": bonus_pct,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a move action: begin multi-tick travel to an adjacent location.
///
/// Sets the agent's `destination_id` and `travel_progress` to the route's
/// travel cost. The agent will be marked as "traveling" and will not be
/// able to take other actions until `travel_progress` reaches 0.
///
/// The tick cycle is responsible for decrementing `travel_progress` each
/// tick and moving the agent to the destination when it reaches 0.
///
/// Awards [`skills::XP_MOVE`] (5) exploration XP on initiating travel.
///
/// Modifies:
/// - Agent destination\_id (set to target)
/// - Agent travel\_progress (set to route cost)
/// - Agent energy (deducted per tick of travel, first tick deducted now)
/// - Agent skill XP (adds exploration XP)
/// - Agent inventory (deducted for toll cost, if route has a toll)
pub fn execute_move(
    agent: &mut AgentState,
    destination: LocationId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let travel_cost = ctx.travel_cost.ok_or_else(|| AgentError::ArithmeticOverflow {
        context: String::from("travel_cost not set in ExecutionContext for move"),
    })?;

    // Deduct toll cost if the route requires payment
    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    if let Some(toll) = &ctx.move_toll_cost {
        for (&resource, &required) in toll {
            inventory::remove_resource(&mut agent.inventory, resource, required)?;
            let neg = i64::from(required).checked_neg().ok_or_else(|| {
                AgentError::ArithmeticOverflow {
                    context: String::from("toll cost negation overflow"),
                }
            })?;
            resource_changes.insert(resource, neg);
        }
    }

    agent.destination_id = Some(destination);
    agent.travel_progress = travel_cost;

    // Deduct first tick of movement energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Move));

    // Award exploration XP
    let xp_gained = skills::XP_MOVE;
    let xp_entry = agent.skill_xp.entry(String::from("exploration")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("exploration XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("exploration"), xp_gained);

    // Build toll detail for the outcome
    let toll_detail: Vec<String> = resource_changes
        .iter()
        .map(|(r, q)| format!("{r:?}: {q}"))
        .collect();

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Move),
            skill_xp,
            details: serde_json::json!({
                "destination": destination.to_string(),
                "travel_ticks": travel_cost,
                "toll_paid": toll_detail,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: true,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Advance travel progress for an agent currently in transit.
///
/// Called during the World Wake phase for agents with `travel_progress > 0`.
/// Decrements `travel_progress` by 1 and deducts movement energy. When
/// progress reaches 0, the agent has arrived at the destination.
///
/// Returns `true` if the agent arrived this tick, `false` if still traveling.
pub const fn advance_travel(agent: &mut AgentState) -> Result<bool, AgentError> {
    if agent.travel_progress == 0 {
        return Ok(false);
    }

    agent.travel_progress = agent.travel_progress.saturating_sub(1);

    // Deduct movement energy for this tick of travel
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Move));

    if agent.travel_progress == 0 {
        // Agent has arrived
        if let Some(dest) = agent.destination_id.take() {
            agent.location_id = dest;
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Execute a communicate action: send a direct message to a co-located agent.
///
/// Creates a [`Message`] with the sender and recipient, and truncates
/// the content to [`MAX_MESSAGE_LENGTH`] characters. The energy cost
/// is deducted from the sender.
///
/// Modifies:
/// - Agent energy (deducted for communication cost)
///
/// Produces:
/// - One [`Message`] in `HandlerResult::messages` for the location board
pub fn execute_communicate(
    agent: &mut AgentState,
    target_agent: AgentId,
    message_content: &str,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Truncate message to max length
    let truncated: String = message_content.chars().take(MAX_MESSAGE_LENGTH).collect();

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Communicate));

    let msg = Message {
        sender_id: agent.agent_id,
        sender_name: ctx.agent_name.clone(),
        recipient_id: Some(target_agent),
        content: truncated.clone(),
        tick: ctx.current_tick,
        is_broadcast: false,
        location_id: agent.location_id,
    };

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Communicate),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "communicate",
                "target": target_agent.to_string(),
                "message_length": truncated.len(),
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: vec![msg],
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a broadcast action: post a message visible to all agents at the location.
///
/// Creates a [`Message`] marked as broadcast, truncated to
/// [`MAX_MESSAGE_LENGTH`] characters. The energy cost is higher than
/// direct communication (5 vs 2).
///
/// Modifies:
/// - Agent energy (deducted for broadcast cost)
///
/// Produces:
/// - One [`Message`] in `HandlerResult::messages` for the location board
pub fn execute_broadcast(
    agent: &mut AgentState,
    message_content: &str,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Truncate message to max length
    let truncated: String = message_content.chars().take(MAX_MESSAGE_LENGTH).collect();

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Broadcast));

    let msg = Message {
        sender_id: agent.agent_id,
        sender_name: ctx.agent_name.clone(),
        recipient_id: None,
        content: truncated.clone(),
        tick: ctx.current_tick,
        is_broadcast: true,
        location_id: agent.location_id,
    };

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Broadcast),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "broadcast",
                "message_length": truncated.len(),
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: vec![msg],
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a teach action: transfer knowledge to a co-located agent.
///
/// The teacher must know the concept, the student must not (validated
/// upstream). The teacher pays 10 energy. Success is determined by
/// the caller (the tick cycle rolls against the teaching probability
/// using [`crate::knowledge::attempt_teach`]).
///
/// This handler only records the energy expenditure and builds the
/// outcome payload. The actual knowledge transfer (calling
/// `KnowledgeBase::learn` on the student) is performed by the tick
/// cycle after success is determined.
///
/// Awards [`skills::XP_TEACH`] (10) teaching XP on success (the caller
/// should only invoke this handler for successful teach attempts).
///
/// Modifies:
/// - Agent energy (deducted for teaching cost)
/// - Agent skill XP (adds teaching XP)
///
/// The `success` field in the returned outcome's `details` is always
/// set to `true` here -- the caller overrides it if the roll fails.
pub fn execute_teach(
    agent: &mut AgentState,
    target_agent: AgentId,
    knowledge: &str,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Teach));

    // Award teaching XP
    let xp_gained = skills::XP_TEACH;
    let xp_entry = agent.skill_xp.entry(String::from("teaching")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("teaching XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("teaching"), xp_gained);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Teach),
            skill_xp,
            details: serde_json::json!({
                "type": "teach",
                "teacher": agent.agent_id.to_string(),
                "student": target_agent.to_string(),
                "knowledge": knowledge,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a reproduce action: initiate reproduction with a consenting partner.
///
/// This handler records the energy expenditure for the *initiating* agent and
/// produces the outcome. The tick cycle is responsible for:
/// - Validating partner consent and vitals (via `reproduction::validate_reproduction`)
/// - Deducting energy from the partner
/// - Creating the child agent (via `AgentManager::create_child_agent`)
/// - Emitting the `AgentBorn` event
///
/// Modifies:
/// - Agent energy (deducted by [`costs::energy_cost(ActionType::Reproduce)`], which is 30)
pub fn execute_reproduce(
    agent: &mut AgentState,
    partner_agent: AgentId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Reproduce));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Reproduce),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "reproduce",
                "initiator": agent.agent_id.to_string(),
                "partner": partner_agent.to_string(),
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a build action: construct a new structure at the agent's location.
///
/// The handler:
/// 1. Looks up the [`StructureBlueprint`] for the requested structure type
/// 2. Deducts all required materials from the agent's inventory
/// 3. Deducts the build energy cost (25)
/// 4. Awards [`skills::XP_BUILD`] (15) building XP
/// 5. Creates a new [`Structure`] and returns it in `structure_built`
///
/// The tick cycle is responsible for adding the structure to the world map
/// and emitting the `StructureBuilt` event.
///
/// Modifies:
/// - Agent inventory (removes material costs)
/// - Agent energy (deducted for build cost)
/// - Agent skill XP (adds building XP)
pub fn execute_build(
    agent: &mut AgentState,
    structure_type: StructureType,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let bp = world_structure::blueprint(structure_type);

    // Deduct materials from inventory
    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    for (&resource, &quantity) in &bp.material_costs {
        inventory::remove_resource(&mut agent.inventory, resource, quantity)?;
        let neg = i64::from(quantity).checked_neg().ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("build material cost negation overflow"),
            }
        })?;
        resource_changes.insert(resource, neg);
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Build));

    // Award building XP
    let xp_gained = skills::XP_BUILD;
    let xp_entry = agent
        .skill_xp
        .entry(String::from("building"))
        .or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("building XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("building"), xp_gained);

    // Create the new structure
    let structure = Structure {
        id: StructureId::new(),
        structure_type,
        subtype: None,
        location_id: agent.location_id,
        builder: agent.agent_id,
        owner: Some(agent.agent_id),
        built_at_tick: ctx.current_tick,
        destroyed_at_tick: None,
        materials_used: bp.material_costs.clone(),
        durability: bp.max_durability,
        max_durability: bp.max_durability,
        decay_per_tick: bp.decay_per_tick,
        capacity: bp.capacity,
        occupants: BTreeSet::new(),
        access_list: None,
        properties: bp.properties,
    };

    let structure_id = structure.id;

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Build),
            skill_xp,
            details: serde_json::json!({
                "type": "build",
                "structure_type": format!("{structure_type:?}"),
                "structure_id": structure_id.to_string(),
                "location": agent.location_id.to_string(),
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: Some(structure),
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a repair action: restore a structure's durability to maximum.
///
/// The handler:
/// 1. Looks up the structure from the execution context
/// 2. Computes the proportional repair cost based on missing durability
/// 3. Deducts repair materials from the agent's inventory
/// 4. Deducts the repair energy cost (15)
/// 5. Awards [`skills::XP_BUILD`] (15) building XP
/// 6. Returns the structure ID in `structure_repaired`
///
/// The tick cycle is responsible for calling [`apply_repair`] on the
/// actual structure in the world map and emitting the `StructureRepaired`
/// event.
///
/// [`apply_repair`]: emergence_world::structure::apply_repair
///
/// Modifies:
/// - Agent inventory (removes proportional repair materials)
/// - Agent energy (deducted for repair cost)
/// - Agent skill XP (adds building XP)
pub fn execute_repair(
    agent: &mut AgentState,
    structure_id: StructureId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Look up the structure at the location
    let structure = ctx
        .structures_at_location
        .get(&structure_id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: format!("structure {structure_id} not found at location for repair"),
        })?;

    // Compute proportional repair cost
    let repair_costs = world_structure::compute_repair_cost(
        &structure.materials_used,
        structure.durability,
        structure.max_durability,
    )
    .map_err(|_world_err| AgentError::ArithmeticOverflow {
        context: String::from("repair cost computation overflow"),
    })?;

    // Deduct repair materials from inventory
    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    for (&resource, &quantity) in &repair_costs {
        inventory::remove_resource(&mut agent.inventory, resource, quantity)?;
        let neg = i64::from(quantity).checked_neg().ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("repair material cost negation overflow"),
            }
        })?;
        resource_changes.insert(resource, neg);
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Repair));

    // Award building XP
    let xp_gained = skills::XP_BUILD;
    let xp_entry = agent
        .skill_xp
        .entry(String::from("building"))
        .or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("building XP overflow in repair"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("building"), xp_gained);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Repair),
            skill_xp,
            details: serde_json::json!({
                "type": "repair",
                "structure_id": structure_id.to_string(),
                "durability_before": structure.durability,
                "durability_after": structure.max_durability,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: Some(structure_id),
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a demolish action: destroy a structure and recover salvage materials.
///
/// The handler:
/// 1. Looks up the structure from the execution context
/// 2. Computes 30% material salvage from original construction costs
/// 3. Adds salvaged materials to the agent's inventory (respecting capacity)
/// 4. Deducts the demolish energy cost (20)
/// 5. Returns the structure ID in `structure_demolished`
///
/// The tick cycle is responsible for removing the structure from the world
/// map and emitting the `StructureDestroyed` event.
///
/// Modifies:
/// - Agent inventory (adds salvaged materials, capped by carry capacity)
/// - Agent energy (deducted for demolish cost)
pub fn execute_demolish(
    agent: &mut AgentState,
    structure_id: StructureId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Look up the structure at the location
    let structure = ctx
        .structures_at_location
        .get(&structure_id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: format!("structure {structure_id} not found at location for demolish"),
        })?;

    // Compute salvage (30% of original materials)
    let salvage = world_structure::compute_salvage(&structure.materials_used).map_err(|_world_err| {
        AgentError::ArithmeticOverflow {
            context: String::from("salvage computation overflow"),
        }
    })?;

    // Add salvaged materials to inventory (capped by carry capacity)
    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    for (&resource, &quantity) in &salvage {
        // Try to add; if it would exceed capacity, add as much as possible.
        // If the agent is overloaded, the extra salvage is lost.
        if inventory::add_resource(
            &mut agent.inventory,
            agent.carry_capacity,
            resource,
            quantity,
        )
        .is_ok()
        {
            resource_changes.insert(resource, i64::from(quantity));
        }
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Demolish));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Demolish),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "demolish",
                "structure_id": structure_id.to_string(),
                "structure_type": format!("{:?}", structure.structure_type),
                "salvaged": salvage.iter().map(|(r, q)| format!("{r:?}: {q}")).collect::<Vec<_>>(),
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: Some(structure_id),
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute an improve-route action: upgrade or repair a route.
///
/// The handler determines whether this is an upgrade or a repair based on
/// the route's current [`PathType`]:
///
/// - **Upgrade**: if the route can be upgraded (not at [`PathType::Highway`]),
///   the handler deducts the material costs from the agent's inventory and
///   signals the caller to apply [`world_route::apply_route_upgrade`] on the
///   actual route in the world map.
/// - **Repair**: if the route is already at the maximum level or the agent
///   wants to restore durability, the handler signals the caller to apply
///   [`world_route::repair_route`].
///
/// In both cases the handler:
/// 1. Deducts energy (30)
/// 2. Awards [`skills::XP_BUILD`] (15) building XP
/// 3. Returns the upgrade/repair details in `HandlerResult`
///
/// The tick cycle is responsible for calling the appropriate route function
/// on the world map and emitting the `RouteImproved` event.
///
/// Modifies:
/// - Agent inventory (removes upgrade materials, if upgrading)
/// - Agent energy (deducted for improve route cost)
/// - Agent skill XP (adds building XP)
pub fn execute_improve_route(
    agent: &mut AgentState,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // The route must have been set on the context by the tick cycle.
    let route = ctx
        .route_to_improve
        .as_ref()
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("route_to_improve not set in ExecutionContext for ImproveRoute"),
        })?;

    let next = world_route::next_path_upgrade(route.path_type);

    // Determine whether to upgrade or repair, and deduct materials if upgrading
    let old_path = route.path_type;
    let (is_upgrade, new_path, materials_used) = if let Some(target) = next {
        // Upgrade path: deduct materials from inventory
        let mat_costs = world_route::upgrade_cost(target).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("upgrade_cost returned None for a valid target PathType"),
            }
        })?;

        for (&resource, &required) in &mat_costs {
            inventory::remove_resource(&mut agent.inventory, resource, required)?;
        }

        (true, target, mat_costs)
    } else {
        // Repair path: no material cost, just restore durability
        (false, old_path, BTreeMap::new())
    };

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::ImproveRoute));

    // Award building XP
    let xp_gained = skills::XP_BUILD;
    let xp_entry = agent
        .skill_xp
        .entry(String::from("building"))
        .or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("building XP overflow in improve_route"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("building"), xp_gained);

    // Build resource changes for the outcome (negative = materials spent)
    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    for (&resource, &quantity) in &materials_used {
        let neg = i64::from(quantity).checked_neg().ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("route material cost negation overflow in outcome"),
            }
        })?;
        resource_changes.insert(resource, neg);
    }

    // Build the JSON details before moving materials_used
    let materials_detail: Vec<String> = materials_used
        .iter()
        .map(|(r, q)| format!("{r:?}: {q}"))
        .collect();

    let details = serde_json::json!({
        "type": if is_upgrade { "upgrade" } else { "repair" },
        "old_path_type": format!("{old_path:?}"),
        "new_path_type": format!("{new_path:?}"),
        "materials_used": materials_detail,
        "tick": ctx.current_tick,
    });

    // Set the appropriate route result field
    let (route_upgraded, route_repaired_val) = if is_upgrade {
        (Some((old_path, new_path, materials_used)), None)
    } else {
        // The actual durability restored is computed by the tick cycle when
        // it calls repair_route(). We signal repair with a sentinel value.
        let estimated_restore = route.max_durability.saturating_sub(route.durability);
        (None, Some(estimated_restore))
    };

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::ImproveRoute),
            skill_xp,
            details,
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded,
        route_repaired: route_repaired_val,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a claim action: take ownership of an unowned or orphaned structure.
///
/// The handler:
/// 1. Looks up the target structure from the execution context
/// 2. Verifies the structure is either unowned or its owner is dead
/// 3. Deducts the claim energy cost (5)
/// 4. Returns the structure ID in `structure_claimed`
///
/// The tick cycle is responsible for updating the structure's `owner` field
/// in world state and emitting the `StructureClaimed` event.
///
/// Modifies:
/// - Agent energy (deducted for claim cost)
pub fn execute_claim(
    agent: &mut AgentState,
    structure_id: StructureId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Look up the structure at the location
    let structure = ctx
        .structures_at_location
        .get(&structure_id)
        .ok_or_else(|| AgentError::GovernanceFailed {
            reason: format!("structure {structure_id} not found at location for claim"),
        })?;

    // Verify the structure is claimable: unowned or owner is dead
    if let Some(owner) = structure.owner
        && !ctx.dead_agents.contains(&owner)
    {
        return Err(AgentError::GovernanceFailed {
            reason: format!(
                "structure {structure_id} is owned by living agent {owner}"
            ),
        });
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Claim));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Claim),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "claim",
                "structure_id": structure_id.to_string(),
                "structure_type": format!("{:?}", structure.structure_type),
                "previous_owner": structure.owner.map(|o| o.to_string()),
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: Some(structure_id),
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a legislate action: create a governance rule for a group.
///
/// The handler:
/// 1. Verifies the agent is a member of the specified group
/// 2. Verifies a `MeetingHall` structure exists at the location
/// 3. Deducts the legislate energy cost (10)
/// 4. Creates a new [`Rule`] and returns it in `rule_created`
///
/// The tick cycle is responsible for storing the rule in the active rules
/// registry and emitting the `RuleCreated` event.
///
/// Modifies:
/// - Agent energy (deducted for legislate cost)
pub fn execute_legislate(
    agent: &mut AgentState,
    rule_name: &str,
    rule_description: &str,
    group_id: GroupId,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Verify agent is a member of the group
    if !ctx.agent_groups.contains(&group_id) {
        return Err(AgentError::GovernanceFailed {
            reason: format!(
                "agent {} is not a member of group {group_id}",
                agent.agent_id
            ),
        });
    }

    // Verify a MeetingHall exists at the location
    let has_meeting_hall = ctx
        .structures_at_location
        .values()
        .any(|s| s.structure_type == StructureType::MeetingHall);

    if !has_meeting_hall {
        return Err(AgentError::GovernanceFailed {
            reason: String::from("no MeetingHall structure at this location"),
        });
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Legislate));

    let rule_id = RuleId::new();

    let rule = Rule {
        id: rule_id,
        group_id,
        creator: agent.agent_id,
        name: String::from(rule_name),
        description: String::from(rule_description),
        created_at_tick: ctx.current_tick,
        active: true,
    };

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Legislate),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "legislate",
                "rule_id": rule_id.to_string(),
                "rule_name": rule_name,
                "group_id": group_id.to_string(),
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: Some(rule),
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute an enforce action: apply consequences for rule violation.
///
/// The handler:
/// 1. Looks up the rule from the execution context
/// 2. Verifies the enforcer is a member of the rule's group
/// 3. Deducts the enforce energy cost (15)
/// 4. Creates [`EnforcementAppliedDetails`] and returns it in `enforcement`
///
/// The tick cycle is responsible for:
/// - Applying relationship penalties (target's relationship with enforcer decreases)
/// - Emitting the `EnforcementApplied` event
/// - Optionally restricting target's access to group-controlled structures
///
/// Modifies:
/// - Agent energy (deducted for enforce cost)
pub fn execute_enforce(
    agent: &mut AgentState,
    target_agent: AgentId,
    rule_id: RuleId,
    consequence: &str,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Look up the rule
    let rule = ctx
        .active_rules
        .get(&rule_id)
        .ok_or_else(|| AgentError::GovernanceFailed {
            reason: format!("rule {rule_id} not found in active rules"),
        })?;

    // Verify enforcer is a member of the rule's group
    if !ctx.agent_groups.contains(&rule.group_id) {
        return Err(AgentError::GovernanceFailed {
            reason: format!(
                "agent {} is not a member of group {} that created rule {rule_id}",
                agent.agent_id, rule.group_id
            ),
        });
    }

    // Deduct energy
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Enforce));

    let enforcement_details = EnforcementAppliedDetails {
        rule_id,
        enforcer: agent.agent_id,
        target: target_agent,
        group_id: rule.group_id,
        consequence: String::from(consequence),
    };

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Enforce),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "enforce",
                "rule_id": rule_id.to_string(),
                "target": target_agent.to_string(),
                "group_id": rule.group_id.to_string(),
                "consequence": consequence,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: Some(enforcement_details),
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

// ---------------------------------------------------------------------------
// Advanced action handlers (Phase 4.2)
// ---------------------------------------------------------------------------

/// Execute a farm-plant action: plant crops on a farm plot at the agent's location.
///
/// Consumes 1 food from inventory (the seed), deducts 20 energy, awards
/// [`skills::XP_FARM_PLANT`] (10) farming XP. The crop will mature after
/// [`farming::DEFAULT_GROWTH_TICKS`] (10) ticks.
pub fn execute_farm_plant(
    agent: &mut AgentState,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    // Find a FarmPlot at this location that has no crops
    let farm_id = ctx
        .structures_at_location
        .iter()
        .find(|(sid, s)| {
            s.structure_type == StructureType::FarmPlot
                && s.durability > 0
                && s.destroyed_at_tick.is_none()
                && !ctx.farm_registry.has_crops(**sid)
        })
        .map(|(id, _)| *id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("no available farm plot at location for planting"),
        })?;

    // Deduct 1 food item as seed
    let seed_food = [
        Resource::FoodBerry,
        Resource::FoodRoot,
        Resource::FoodFish,
        Resource::FoodMeat,
        Resource::FoodFarmed,
        Resource::FoodCooked,
    ]
    .into_iter()
    .find(|f| inventory::has_resource(&agent.inventory, *f, 1))
    .ok_or(AgentError::InsufficientResource {
        resource: Resource::FoodBerry,
        requested: 1,
        available: 0,
    })?;

    inventory::remove_resource(&mut agent.inventory, seed_food, 1)?;

    let growth_ticks = farming::DEFAULT_GROWTH_TICKS;
    let success = ctx
        .farm_registry
        .plant(farm_id, ctx.current_tick, growth_ticks);
    if !success {
        return Err(AgentError::ArithmeticOverflow {
            context: String::from("farm_registry.plant failed (already planted or overflow)"),
        });
    }

    let mature_at_tick = ctx.current_tick.checked_add(growth_ticks).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("mature_at_tick overflow"),
        }
    })?;

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::FarmPlant));

    let xp_gained = skills::XP_FARM_PLANT;
    let xp_entry = agent.skill_xp.entry(String::from("farming")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("farming XP overflow in farm_plant"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("farming"), xp_gained);

    let mut resource_changes = BTreeMap::new();
    resource_changes.insert(seed_food, -1);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::FarmPlant),
            skill_xp,
            details: serde_json::json!({
                "type": "farm_plant",
                "farm_id": farm_id.to_string(),
                "seed_food": format!("{seed_food:?}"),
                "mature_at_tick": mature_at_tick,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: Some((farm_id, mature_at_tick)),
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a farm-harvest action: harvest mature crops from a farm plot.
///
/// Yields [`farming::BASE_HARVEST_YIELD`] (5) + farming skill bonus units of
/// [`Resource::FoodFarmed`]. Deducts 10 energy, awards
/// [`skills::XP_FARM_HARVEST`] (10) farming XP.
pub fn execute_farm_harvest(
    agent: &mut AgentState,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let farm_id = ctx
        .structures_at_location
        .iter()
        .find(|(sid, s)| {
            s.structure_type == StructureType::FarmPlot
                && s.durability > 0
                && s.destroyed_at_tick.is_none()
                && ctx.farm_registry.is_harvestable(**sid, ctx.current_tick)
        })
        .map(|(id, _)| *id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("no harvestable farm plot at location"),
        })?;

    let skill_level = agent.skills.get("farming").copied().unwrap_or(0);
    let yield_amount =
        farming::harvest_yield(skill_level).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("harvest yield overflow"),
        })?;

    inventory::add_resource(
        &mut agent.inventory,
        agent.carry_capacity,
        Resource::FoodFarmed,
        yield_amount,
    )?;

    ctx.farm_registry.harvest(farm_id);
    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::FarmHarvest));

    let xp_gained = skills::XP_FARM_HARVEST;
    let xp_entry = agent.skill_xp.entry(String::from("farming")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("farming XP overflow in farm_harvest"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("farming"), xp_gained);

    let mut resource_changes = BTreeMap::new();
    resource_changes.insert(Resource::FoodFarmed, i64::from(yield_amount));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::FarmHarvest),
            skill_xp,
            details: serde_json::json!({
                "type": "farm_harvest",
                "farm_id": farm_id.to_string(),
                "yield": yield_amount,
                "skill_level": skill_level,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: Some(farm_id),
        library_write: None,
        library_read: None,
    })
}

/// Execute a craft action: produce a tool, advanced tool, or medicine at a workshop.
///
/// Deducts 15 energy, awards [`skills::XP_CRAFT`] (10) crafting XP.
pub fn execute_craft(
    agent: &mut AgentState,
    output: Resource,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let recipe = crafting::recipe_for(output).ok_or_else(|| AgentError::ArithmeticOverflow {
        context: format!("no crafting recipe for {output:?}"),
    })?;

    let mut resource_changes: BTreeMap<Resource, i64> = BTreeMap::new();
    for (&resource, &quantity) in &recipe.inputs {
        inventory::remove_resource(&mut agent.inventory, resource, quantity)?;
        let neg = i64::from(quantity).checked_neg().ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("craft input negation overflow"),
            }
        })?;
        resource_changes.insert(resource, neg);
    }

    inventory::add_resource(
        &mut agent.inventory,
        agent.carry_capacity,
        recipe.output,
        recipe.output_quantity,
    )?;
    resource_changes.insert(recipe.output, i64::from(recipe.output_quantity));

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Craft));

    let xp_gained = skills::XP_CRAFT;
    let xp_entry = agent.skill_xp.entry(String::from("crafting")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("crafting XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("crafting"), xp_gained);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Craft),
            skill_xp,
            details: serde_json::json!({
                "type": "craft",
                "output": format!("{:?}", recipe.output),
                "quantity": recipe.output_quantity,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a mine action: extract ore from the location.
///
/// Yield is [`costs::BASE_MINE_YIELD`] (2) + mining skill bonus, capped by
/// available ore. Deducts 20 energy, awards [`skills::XP_MINE`] (10) mining XP.
pub fn execute_mine(
    agent: &mut AgentState,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let skill_level = agent.skills.get("mining").copied().unwrap_or(0);
    let target_yield =
        effects::mining_yield(costs::BASE_MINE_YIELD, skill_level).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("mine yield overflow"),
            }
        })?;

    let available = ctx
        .location_resources
        .get(&Resource::Ore)
        .copied()
        .unwrap_or(0);
    let actual = target_yield.min(available);

    inventory::add_resource(
        &mut agent.inventory,
        agent.carry_capacity,
        Resource::Ore,
        actual,
    )?;

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Mine));

    if let Some(loc_avail) = ctx.location_resources.get_mut(&Resource::Ore) {
        *loc_avail = loc_avail.saturating_sub(actual);
    }

    let xp_gained = skills::XP_MINE;
    let xp_entry = agent.skill_xp.entry(String::from("mining")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("mining XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("mining"), xp_gained);

    let mut resource_changes = BTreeMap::new();
    resource_changes.insert(Resource::Ore, i64::from(actual));

    let mut location_deltas = BTreeMap::new();
    location_deltas.insert(Resource::Ore, actual);

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Mine),
            skill_xp,
            details: serde_json::json!({
                "type": "mine",
                "yield": actual,
                "skill_level": skill_level,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: location_deltas,
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a smelt action: convert ore into metal at a forge.
///
/// Consumes 2 Ore + 1 Wood, produces 1 Metal.
/// Deducts 20 energy, awards [`skills::XP_SMELT`] (10) smelting XP.
pub fn execute_smelt(
    agent: &mut AgentState,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    inventory::remove_resource(&mut agent.inventory, Resource::Ore, costs::SMELT_ORE_INPUT)?;
    inventory::remove_resource(&mut agent.inventory, Resource::Wood, costs::SMELT_WOOD_INPUT)?;
    inventory::add_resource(
        &mut agent.inventory,
        agent.carry_capacity,
        Resource::Metal,
        costs::SMELT_METAL_OUTPUT,
    )?;

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Smelt));

    let xp_gained = skills::XP_SMELT;
    let xp_entry = agent.skill_xp.entry(String::from("smelting")).or_insert(0);
    *xp_entry = xp_entry.checked_add(xp_gained).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("smelting XP overflow"),
        }
    })?;

    let mut skill_xp = BTreeMap::new();
    skill_xp.insert(String::from("smelting"), xp_gained);

    let ore_neg = i64::from(costs::SMELT_ORE_INPUT).checked_neg().ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("smelt ore negation overflow"),
        }
    })?;
    let wood_neg = i64::from(costs::SMELT_WOOD_INPUT)
        .checked_neg()
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("smelt wood negation overflow"),
        })?;

    let mut resource_changes = BTreeMap::new();
    resource_changes.insert(Resource::Ore, ore_neg);
    resource_changes.insert(Resource::Wood, wood_neg);
    resource_changes.insert(Resource::Metal, i64::from(costs::SMELT_METAL_OUTPUT));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes,
            energy_spent: costs::energy_cost(ActionType::Smelt),
            skill_xp,
            details: serde_json::json!({
                "type": "smelt",
                "ore_consumed": costs::SMELT_ORE_INPUT,
                "wood_consumed": costs::SMELT_WOOD_INPUT,
                "metal_produced": costs::SMELT_METAL_OUTPUT,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    })
}

/// Execute a write action: persist a knowledge concept to a library.
///
/// Deducts 5 energy. The concept is added to the library's knowledge set.
pub fn execute_write(
    agent: &mut AgentState,
    knowledge: &str,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let library_id = ctx
        .structures_at_location
        .iter()
        .find(|(_, s)| {
            s.structure_type == StructureType::Library
                && s.durability > 0
                && s.destroyed_at_tick.is_none()
        })
        .map(|(id, _)| *id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("no library at location for write"),
        })?;

    ctx.library_knowledge
        .entry(library_id)
        .or_default()
        .insert(String::from(knowledge));

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Write));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Write),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "write",
                "library_id": library_id.to_string(),
                "knowledge": knowledge,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: Some((library_id, String::from(knowledge))),
        library_read: None,
    })
}

/// Execute a read action: acquire a knowledge concept from a library.
///
/// Deducts 5 energy. 100% success rate (unlike teaching).
/// The tick cycle applies the knowledge via
/// [`KnowledgeBase::learn`](crate::knowledge::KnowledgeBase::learn).
pub fn execute_read(
    agent: &mut AgentState,
    knowledge: &str,
    ctx: &ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    let library_id = ctx
        .structures_at_location
        .iter()
        .find(|(_, s)| {
            s.structure_type == StructureType::Library
                && s.durability > 0
                && s.destroyed_at_tick.is_none()
        })
        .map(|(id, _)| *id)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("no library at location for read"),
        })?;

    vitals::apply_energy_cost(agent, costs::energy_cost(ActionType::Read));

    Ok(HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: costs::energy_cost(ActionType::Read),
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({
                "type": "read",
                "library_id": library_id.to_string(),
                "knowledge": knowledge,
                "tick": ctx.current_tick,
            }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: Some((library_id, String::from(knowledge))),
    })
}

/// Execute a no-action: the agent did nothing this tick (timeout or explicit).
///
/// No side effects. Returns a trivial outcome.
pub fn execute_no_action(_agent: &AgentState) -> HandlerResult {
    HandlerResult {
        outcome: ActionOutcome {
            resource_changes: BTreeMap::new(),
            energy_spent: 0,
            skill_xp: BTreeMap::new(),
            details: serde_json::json!({ "action": "none" }),
        },
        location_resource_deltas: BTreeMap::new(),
        began_travel: false,
        messages: Vec::new(),
        structure_built: None,
        structure_repaired: None,
        structure_demolished: None,
        route_upgraded: None,
        route_repaired: None,
        structure_claimed: None,
        rule_created: None,
        enforcement: None,
        farm_planted: None,
        farm_harvested: None,
        library_write: None,
        library_read: None,
    }
}

/// Dispatch an action to the appropriate handler.
///
/// This is the main entry point for action execution after validation.
/// Advanced actions (build, trade, craft, etc.) return `NoAction` outcomes
/// in Phase 2 -- they will be implemented in Phase 3+.
pub fn execute_action(
    action_type: ActionType,
    params: &ActionParameters,
    agent: &mut AgentState,
    config: &VitalsConfig,
    ctx: &mut ExecutionContext,
) -> Result<HandlerResult, AgentError> {
    match (action_type, params) {
        (ActionType::Gather, ActionParameters::Gather { resource }) => {
            execute_gather(agent, *resource, config, ctx)
        }
        (ActionType::Eat, ActionParameters::Eat { food_type }) => {
            execute_eat(agent, *food_type, config)
        }
        (ActionType::Drink, ActionParameters::Drink) => execute_drink(agent, config, ctx),
        (ActionType::Rest, ActionParameters::Rest) => execute_rest(agent, config, ctx),
        (ActionType::Move, ActionParameters::Move { destination }) => {
            execute_move(agent, *destination, ctx)
        }
        (ActionType::Communicate, ActionParameters::Communicate { target_agent, message }) => {
            execute_communicate(agent, *target_agent, message, ctx)
        }
        (ActionType::Broadcast, ActionParameters::Broadcast { message }) => {
            execute_broadcast(agent, message, ctx)
        }
        (ActionType::Teach, ActionParameters::Teach { target_agent, knowledge }) => {
            execute_teach(agent, *target_agent, knowledge, ctx)
        }
        (ActionType::Reproduce, ActionParameters::Reproduce { partner_agent }) => {
            execute_reproduce(agent, *partner_agent, ctx)
        }
        (ActionType::Build, ActionParameters::Build { structure_type }) => {
            execute_build(agent, *structure_type, ctx)
        }
        (ActionType::Repair, ActionParameters::Repair { structure_id }) => {
            execute_repair(agent, *structure_id, ctx)
        }
        (ActionType::Demolish, ActionParameters::Demolish { structure_id }) => {
            execute_demolish(agent, *structure_id, ctx)
        }
        (ActionType::ImproveRoute, ActionParameters::ImproveRoute { .. }) => {
            execute_improve_route(agent, ctx)
        }
        (ActionType::Claim, ActionParameters::Claim { structure_id }) => {
            execute_claim(agent, *structure_id, ctx)
        }
        (
            ActionType::Legislate,
            ActionParameters::Legislate {
                rule_name,
                rule_description,
                group_id,
            },
        ) => execute_legislate(agent, rule_name, rule_description, *group_id, ctx),
        (
            ActionType::Enforce,
            ActionParameters::Enforce {
                target_agent,
                rule_id,
                consequence,
            },
        ) => execute_enforce(agent, *target_agent, *rule_id, consequence, ctx),
        (ActionType::FarmPlant, ActionParameters::FarmPlant) => {
            execute_farm_plant(agent, ctx)
        }
        (ActionType::FarmHarvest, ActionParameters::FarmHarvest) => {
            execute_farm_harvest(agent, ctx)
        }
        (ActionType::Craft, ActionParameters::Craft { output }) => {
            execute_craft(agent, *output, ctx)
        }
        (ActionType::Mine, ActionParameters::Mine) => execute_mine(agent, ctx),
        (ActionType::Smelt, ActionParameters::Smelt) => execute_smelt(agent, ctx),
        (ActionType::Write, ActionParameters::Write { knowledge }) => {
            execute_write(agent, knowledge, ctx)
        }
        (ActionType::Read, ActionParameters::Read { knowledge }) => {
            execute_read(agent, knowledge, ctx)
        }
        (ActionType::NoAction, ActionParameters::NoAction) => Ok(execute_no_action(agent)),
        _ => {
            // Remaining action types (e.g. TradeAccept, TradeReject, FormGroup,
            // Steal, Attack, Propose, Vote, Marry, Divorce, Conspire, Pray)
            // are handled externally by the tick cycle or are not yet wired.
            // Freeform actions are routed through the feasibility evaluator
            // in emergence-core before reaching execution.
            Ok(execute_no_action(agent))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_types::{AgentId, LocationId, Resource, StructureId, StructureType};

    use super::*;

    fn make_agent(energy: u32) -> AgentState {
        AgentState {
            agent_id: AgentId::new(),
            energy,
            health: 100,
            hunger: 0,
            age: 0,
            born_at_tick: 0,
            location_id: LocationId::new(),
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

    fn make_exec_ctx() -> ExecutionContext {
        let mut resources = BTreeMap::new();
        resources.insert(Resource::Wood, 50);
        resources.insert(Resource::Water, 100);
        ExecutionContext {
            location_resources: resources,
            is_sheltered: false,
            shelter_bonus_pct: 100,
            travel_cost: None,
            move_destination: None,
            current_tick: 1,
            agent_name: String::from("TestAgent"),
            structures_at_location: BTreeMap::new(),
            route_to_improve: None,
            move_toll_cost: None,
            dead_agents: BTreeSet::new(),
            agent_groups: BTreeSet::new(),
            active_rules: BTreeMap::new(),
            farm_registry: farming::FarmRegistry::new(),
            library_knowledge: BTreeMap::new(),
        }
    }

    #[test]
    fn gather_adds_to_inventory() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Base yield is 3 (no skill bonus)
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(3));
        assert_eq!(hr.location_resource_deltas.get(&Resource::Wood).copied(), Some(3));
        assert_eq!(hr.outcome.energy_spent, 10);
    }

    #[test]
    fn gather_caps_at_available() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        // Only 2 wood available
        ctx.location_resources.insert(Resource::Wood, 2);

        let result = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(2));
    }

    #[test]
    fn gather_with_skill_bonus() {
        let mut agent = make_agent(80);
        agent.skills.insert(String::from("gathering"), 4);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx);
        assert!(result.is_ok());
        // Yield: 3 + 4/2 = 3 + 2 = 5
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(5));
    }

    #[test]
    fn eat_reduces_hunger_and_removes_food() {
        let mut agent = make_agent(50);
        agent.hunger = 60;
        agent.inventory.insert(Resource::FoodBerry, 5);
        let config = VitalsConfig::default();

        let result = execute_eat(&mut agent, Resource::FoodBerry, &config);
        assert!(result.is_ok());
        // Berry: hunger -20, energy +5
        assert_eq!(agent.hunger, 40);
        assert_eq!(agent.energy, 55);
        assert_eq!(agent.inventory.get(&Resource::FoodBerry).copied(), Some(4));
    }

    #[test]
    fn drink_from_location() {
        let mut agent = make_agent(50);
        agent.hunger = 20;
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_drink(&mut agent, &config, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();
        // Drank from location, not inventory
        assert_eq!(hr.location_resource_deltas.get(&Resource::Water).copied(), Some(1));
        assert_eq!(agent.hunger, 15); // 20 - 5
        assert_eq!(agent.energy, 55); // 50 + 5
    }

    #[test]
    fn drink_from_inventory_when_no_location_water() {
        let mut agent = make_agent(50);
        agent.inventory.insert(Resource::Water, 3);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.location_resources.remove(&Resource::Water);

        let result = execute_drink(&mut agent, &config, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(agent.inventory.get(&Resource::Water).copied(), Some(2));
    }

    #[test]
    fn rest_recovers_energy() {
        let mut agent = make_agent(20);
        let config = VitalsConfig::default();
        let ctx = make_exec_ctx();

        let result = execute_rest(&mut agent, &config, &ctx);
        assert!(result.is_ok());
        // Rest recovery: 30 (no shelter bonus)
        assert_eq!(agent.energy, 50);
    }

    #[test]
    fn rest_with_shelter_bonus() {
        let mut agent = make_agent(20);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.is_sheltered = true;
        ctx.shelter_bonus_pct = 150;

        let result = execute_rest(&mut agent, &config, &ctx);
        assert!(result.is_ok());
        // Rest recovery: 30 * 150 / 100 = 45
        assert_eq!(agent.energy, 65);
    }

    #[test]
    fn move_sets_travel_state() {
        let mut agent = make_agent(80);
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(5);
        ctx.move_destination = Some(dest);

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_ok());
        assert_eq!(agent.destination_id, Some(dest));
        assert_eq!(agent.travel_progress, 5);
        // First tick energy deducted
        assert_eq!(agent.energy, 65); // 80 - 15
    }

    #[test]
    fn advance_travel_decrements() {
        let mut agent = make_agent(80);
        let dest = LocationId::new();
        agent.destination_id = Some(dest);
        agent.travel_progress = 3;

        let arrived = advance_travel(&mut agent);
        assert!(arrived.is_ok());
        assert!(!arrived.unwrap());
        assert_eq!(agent.travel_progress, 2);

        let arrived = advance_travel(&mut agent);
        assert!(arrived.is_ok());
        assert!(!arrived.unwrap());
        assert_eq!(agent.travel_progress, 1);

        let arrived = advance_travel(&mut agent);
        assert!(arrived.is_ok());
        assert!(arrived.unwrap()); // Arrived!
        assert_eq!(agent.travel_progress, 0);
        assert_eq!(agent.destination_id, None);
        assert_eq!(agent.location_id, dest);
    }

    #[test]
    fn advance_travel_no_op_when_not_traveling() {
        let mut agent = make_agent(80);
        let arrived = advance_travel(&mut agent);
        assert!(arrived.is_ok());
        assert!(!arrived.unwrap());
    }

    #[test]
    fn no_action_has_no_effects() {
        let agent = make_agent(80);
        let result = execute_no_action(&agent);
        assert_eq!(result.outcome.energy_spent, 0);
        assert!(result.location_resource_deltas.is_empty());
    }

    #[test]
    fn dispatch_gather_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        assert!(agent.inventory.get(&Resource::Wood).is_some());
    }

    #[test]
    fn communicate_produces_direct_message() {
        let mut agent = make_agent(80);
        let target = AgentId::new();
        let ctx = make_exec_ctx();

        let result = execute_communicate(
            &mut agent,
            target,
            "Hello friend!",
            &ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Energy spent: 2
        assert_eq!(hr.outcome.energy_spent, 2);
        assert_eq!(agent.energy, 78); // 80 - 2

        // One message produced
        assert_eq!(hr.messages.len(), 1);
        let msg = &hr.messages[0];
        assert_eq!(msg.sender_id, agent.agent_id);
        assert_eq!(msg.recipient_id, Some(target));
        assert!(!msg.is_broadcast);
        assert_eq!(msg.content, "Hello friend!");
        assert_eq!(msg.tick, 1);
    }

    #[test]
    fn communicate_truncates_long_message() {
        let mut agent = make_agent(80);
        let target = AgentId::new();
        let ctx = make_exec_ctx();

        // Create a message longer than 500 characters
        let long_message: String = "a".repeat(600);
        let result = execute_communicate(
            &mut agent,
            target,
            &long_message,
            &ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.messages[0].content.len(), 500);
    }

    #[test]
    fn broadcast_produces_broadcast_message() {
        let mut agent = make_agent(80);
        let ctx = make_exec_ctx();

        let result = execute_broadcast(
            &mut agent,
            "Anyone want to trade?",
            &ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Energy spent: 5
        assert_eq!(hr.outcome.energy_spent, 5);
        assert_eq!(agent.energy, 75); // 80 - 5

        // One broadcast message
        assert_eq!(hr.messages.len(), 1);
        let msg = &hr.messages[0];
        assert!(msg.is_broadcast);
        assert_eq!(msg.recipient_id, None);
        assert_eq!(msg.content, "Anyone want to trade?");
    }

    #[test]
    fn dispatch_communicate_via_execute_action() {
        let mut agent = make_agent(80);
        let target = AgentId::new();
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Communicate,
            &ActionParameters::Communicate {
                target_agent: target,
                message: String::from("Hi there"),
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.messages.len(), 1);
        assert!(!hr.messages[0].is_broadcast);
    }

    #[test]
    fn dispatch_broadcast_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Broadcast,
            &ActionParameters::Broadcast {
                message: String::from("Greetings everyone"),
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.messages.len(), 1);
        assert!(hr.messages[0].is_broadcast);
    }

    // -----------------------------------------------------------------------
    // Skill XP awards (Phase 3.5.3)
    // -----------------------------------------------------------------------

    #[test]
    fn gather_awards_gathering_xp() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Should award XP_GATHER (10) gathering XP
        assert_eq!(
            agent.skill_xp.get("gathering").copied(),
            Some(skills::XP_GATHER)
        );
        assert_eq!(
            hr.outcome.skill_xp.get("gathering").copied(),
            Some(skills::XP_GATHER)
        );
    }

    #[test]
    fn move_awards_exploration_xp() {
        let mut agent = make_agent(80);
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(3);
        ctx.move_destination = Some(dest);

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Should award XP_MOVE (5) exploration XP
        assert_eq!(
            agent.skill_xp.get("exploration").copied(),
            Some(skills::XP_MOVE)
        );
        assert_eq!(
            hr.outcome.skill_xp.get("exploration").copied(),
            Some(skills::XP_MOVE)
        );
    }

    #[test]
    fn teach_awards_teaching_xp() {
        let mut agent = make_agent(80);
        let target = AgentId::new();
        let ctx = make_exec_ctx();

        let result = execute_teach(&mut agent, target, "agriculture", &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Should award XP_TEACH (10) teaching XP
        assert_eq!(
            agent.skill_xp.get("teaching").copied(),
            Some(skills::XP_TEACH)
        );
        assert_eq!(
            hr.outcome.skill_xp.get("teaching").copied(),
            Some(skills::XP_TEACH)
        );
    }

    #[test]
    fn gather_xp_accumulates_over_multiple_actions() {
        let mut agent = make_agent(100);
        let config = VitalsConfig::default();

        // Gather twice
        let mut ctx = make_exec_ctx();
        let _ = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx);
        let mut ctx2 = make_exec_ctx();
        let _ = execute_gather(&mut agent, Resource::Wood, &config, &mut ctx2);

        // Should have 2 * XP_GATHER = 20
        let expected = skills::XP_GATHER.checked_mul(2).unwrap();
        assert_eq!(agent.skill_xp.get("gathering").copied(), Some(expected));
    }

    #[test]
    fn gather_skill_effect_modifies_yield() {
        // Level 0 agent: base yield only
        let mut agent_low = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx_low = make_exec_ctx();
        let _ = execute_gather(&mut agent_low, Resource::Wood, &config, &mut ctx_low);
        let low_yield = agent_low.inventory.get(&Resource::Wood).copied().unwrap();
        assert_eq!(low_yield, 3); // BASE_GATHER_YIELD = 3

        // Level 10 agent: base + 10/2 = 3 + 5 = 8
        let mut agent_high = make_agent(80);
        agent_high.skills.insert(String::from("gathering"), 10);
        let mut ctx_high = make_exec_ctx();
        let _ = execute_gather(&mut agent_high, Resource::Wood, &config, &mut ctx_high);
        let high_yield = agent_high.inventory.get(&Resource::Wood).copied().unwrap();
        assert_eq!(high_yield, 8);
    }

    // -----------------------------------------------------------------------
    // ImproveRoute handler (Phase 4.3)
    // -----------------------------------------------------------------------

    fn make_test_route(from: LocationId, to: LocationId, path: PathType) -> Route {
        use emergence_types::RouteId;
        use rust_decimal::Decimal;

        Route {
            id: RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: world_route::base_cost_for_path_type(path),
            path_type: path,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: None,
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    #[test]
    fn improve_route_upgrade_deducts_materials() {
        let mut agent = make_agent(80);
        // None -> DirtTrail costs 10 wood
        agent.inventory.insert(Resource::Wood, 15);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let route = make_test_route(from_loc, to_loc, PathType::None);

        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // 15 - 10 = 5 wood remaining
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(5));
        // Energy deducted: 30
        assert_eq!(hr.outcome.energy_spent, 30);
        assert_eq!(agent.energy, 50); // 80 - 30

        // Route upgraded is set with correct types
        assert!(hr.route_upgraded.is_some());
        let (old, new, mats) = hr.route_upgraded.unwrap();
        assert_eq!(old, PathType::None);
        assert_eq!(new, PathType::DirtTrail);
        assert_eq!(mats.get(&Resource::Wood).copied(), Some(10));

        // No route repair
        assert!(hr.route_repaired.is_none());
    }

    #[test]
    fn improve_route_upgrade_awards_building_xp() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let route = make_test_route(from_loc, to_loc, PathType::None);

        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Should award XP_BUILD (15) building XP
        assert_eq!(
            agent.skill_xp.get("building").copied(),
            Some(skills::XP_BUILD)
        );
        assert_eq!(
            hr.outcome.skill_xp.get("building").copied(),
            Some(skills::XP_BUILD)
        );
    }

    #[test]
    fn improve_route_repair_restores_durability_estimate() {
        let mut agent = make_agent(80);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let mut route = make_test_route(from_loc, to_loc, PathType::Highway);
        route.durability = 40; // Damaged

        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Should be repair, not upgrade
        assert!(hr.route_upgraded.is_none());
        assert!(hr.route_repaired.is_some());
        // Estimated restore: 100 - 40 = 60
        assert_eq!(hr.route_repaired, Some(60));

        // No materials deducted for repair
        assert!(hr.outcome.resource_changes.is_empty());
        // Energy still deducted
        assert_eq!(hr.outcome.energy_spent, 30);
    }

    #[test]
    fn improve_route_worn_path_upgrade_costs() {
        let mut agent = make_agent(80);
        // WornPath -> Road costs 50 wood, 30 stone
        agent.inventory.insert(Resource::Wood, 50);
        agent.inventory.insert(Resource::Stone, 30);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let route = make_test_route(from_loc, to_loc, PathType::WornPath);

        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // All materials consumed
        assert_eq!(agent.inventory.get(&Resource::Wood), None);
        assert_eq!(agent.inventory.get(&Resource::Stone), None);

        let (old, new, _) = hr.route_upgraded.unwrap();
        assert_eq!(old, PathType::WornPath);
        assert_eq!(new, PathType::Road);
    }

    #[test]
    fn improve_route_insufficient_materials_fails() {
        let mut agent = make_agent(80);
        // None -> DirtTrail costs 10 wood, agent has only 5
        agent.inventory.insert(Resource::Wood, 5);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let route = make_test_route(from_loc, to_loc, PathType::None);

        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_err());
        // Inventory should be unchanged on failure (remove_resource is atomic per call,
        // and DirtTrail only has one resource type so the first deduction fails)
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(5));
    }

    #[test]
    fn improve_route_no_route_fails() {
        let mut agent = make_agent(80);
        let ctx = make_exec_ctx(); // route_to_improve is None

        let result = execute_improve_route(&mut agent, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_improve_route_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let from_loc = agent.location_id;
        let to_loc = LocationId::new();
        let route = make_test_route(from_loc, to_loc, PathType::None);

        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.route_to_improve = Some(route);

        let result = execute_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: to_loc,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.route_upgraded.is_some());
    }

    // -----------------------------------------------------------------------
    // Construction handlers (Phase 4.1)
    // -----------------------------------------------------------------------

    fn make_test_structure(
        st: StructureType,
        location_id: LocationId,
        owner: Option<AgentId>,
    ) -> Structure {
        let bp = world_structure::blueprint(st);
        Structure {
            id: StructureId::new(),
            structure_type: st,
            subtype: None,
            location_id,
            builder: owner.unwrap_or_else(AgentId::new),
            owner,
            built_at_tick: 1,
            destroyed_at_tick: None,
            materials_used: bp.material_costs.clone(),
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        }
    }

    #[test]
    fn build_campfire_deducts_materials_and_creates_structure() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let ctx = make_exec_ctx();

        let result = execute_build(&mut agent, StructureType::Campfire, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(7));
        assert_eq!(agent.energy, 55);

        assert!(hr.structure_built.is_some());
        let built = hr.structure_built.unwrap();
        assert_eq!(built.structure_type, StructureType::Campfire);
        assert_eq!(built.durability, 50);
        assert_eq!(built.owner, Some(agent.agent_id));
        assert_eq!(built.location_id, agent.location_id);

        assert_eq!(
            hr.outcome.resource_changes.get(&Resource::Wood).copied(),
            Some(-3)
        );
        assert_eq!(hr.outcome.energy_spent, 25);
        assert_eq!(
            hr.outcome.skill_xp.get("building").copied(),
            Some(skills::XP_BUILD)
        );
        assert_eq!(
            agent.skill_xp.get("building").copied(),
            Some(skills::XP_BUILD)
        );
    }

    #[test]
    fn build_basic_hut_requires_wood_and_stone() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 25);
        agent.inventory.insert(Resource::Stone, 15);
        let ctx = make_exec_ctx();

        let result = execute_build(&mut agent, StructureType::BasicHut, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(5));
        assert_eq!(agent.inventory.get(&Resource::Stone).copied(), Some(5));
        assert!(hr.structure_built.is_some());
        let built = hr.structure_built.unwrap();
        assert_eq!(built.structure_type, StructureType::BasicHut);
        assert_eq!(built.max_durability, 100);
    }

    #[test]
    fn build_fails_insufficient_materials() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 1);
        let ctx = make_exec_ctx();

        let result = execute_build(&mut agent, StructureType::Campfire, &ctx);
        assert!(result.is_err());
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(1));
    }

    #[test]
    fn repair_deducts_proportional_materials() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 20);
        agent.inventory.insert(Resource::Stone, 10);

        let mut hut = make_test_structure(
            StructureType::BasicHut,
            agent.location_id,
            Some(agent.agent_id),
        );
        hut.durability = 50;
        let hut_id = hut.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(hut_id, hut);

        let result = execute_repair(&mut agent, hut_id, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(10));
        assert_eq!(agent.inventory.get(&Resource::Stone).copied(), Some(5));
        assert_eq!(hr.outcome.energy_spent, 15);
        assert_eq!(hr.structure_repaired, Some(hut_id));
        assert_eq!(
            agent.skill_xp.get("building").copied(),
            Some(skills::XP_BUILD)
        );
    }

    #[test]
    fn repair_at_full_durability_costs_nothing() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);

        let campfire = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            Some(agent.agent_id),
        );
        let cf_id = campfire.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(cf_id, campfire);

        let result = execute_repair(&mut agent, cf_id, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(10));
        assert!(hr.outcome.resource_changes.is_empty());
    }

    #[test]
    fn demolish_adds_salvage_to_inventory() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 5);

        let hut = make_test_structure(
            StructureType::BasicHut,
            agent.location_id,
            Some(agent.agent_id),
        );
        let hut_id = hut.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(hut_id, hut);

        let result = execute_demolish(&mut agent, hut_id, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(11));
        assert_eq!(agent.inventory.get(&Resource::Stone).copied(), Some(3));
        assert_eq!(hr.outcome.energy_spent, 20);
        assert_eq!(hr.structure_demolished, Some(hut_id));
        assert_eq!(agent.energy, 60);
    }

    #[test]
    fn demolish_respects_carry_capacity() {
        let mut agent = make_agent(80);
        agent.carry_capacity = 10;
        agent.inventory.insert(Resource::Wood, 8);

        let hut = make_test_structure(
            StructureType::BasicHut,
            agent.location_id,
            Some(agent.agent_id),
        );
        let hut_id = hut.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(hut_id, hut);

        let result = execute_demolish(&mut agent, hut_id, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        let total_load: u32 = agent.inventory.values().sum();
        assert!(total_load <= agent.carry_capacity);
        assert_eq!(hr.structure_demolished, Some(hut_id));
    }

    #[test]
    fn dispatch_build_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: StructureType::Campfire,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.structure_built.is_some());
    }

    #[test]
    fn dispatch_repair_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let mut campfire = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            Some(agent.agent_id),
        );
        campfire.durability = 25;
        let cf_id = campfire.id;
        ctx.structures_at_location.insert(cf_id, campfire);

        let result = execute_action(
            ActionType::Repair,
            &ActionParameters::Repair {
                structure_id: cf_id,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.structure_repaired, Some(cf_id));
    }

    #[test]
    fn dispatch_demolish_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let campfire = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            Some(agent.agent_id),
        );
        let cf_id = campfire.id;
        ctx.structures_at_location.insert(cf_id, campfire);

        let result = execute_action(
            ActionType::Demolish,
            &ActionParameters::Demolish {
                structure_id: cf_id,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.structure_demolished, Some(cf_id));
    }

    // -----------------------------------------------------------------------
    // Move toll deduction (Phase 4.3.2)
    // -----------------------------------------------------------------------

    #[test]
    fn move_with_toll_deducts_resources() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(3);
        ctx.move_destination = Some(dest);
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        ctx.move_toll_cost = Some(toll);

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Toll deducted: 10 - 5 = 5
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(5));
        // Resource changes show toll
        assert_eq!(hr.outcome.resource_changes.get(&Resource::Wood).copied(), Some(-5));
        // Energy still deducted
        assert_eq!(hr.outcome.energy_spent, 15);
        // Travel initiated
        assert!(hr.began_travel);
        assert_eq!(agent.travel_progress, 3);
    }

    #[test]
    fn move_without_toll_no_resource_change() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(3);
        ctx.move_destination = Some(dest);
        // No toll
        ctx.move_toll_cost = None;

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Inventory unchanged
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(10));
        // No resource changes
        assert!(hr.outcome.resource_changes.is_empty());
    }

    #[test]
    fn move_with_toll_insufficient_resources_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 2); // only 2, need 5
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(3);
        ctx.move_destination = Some(dest);
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        ctx.move_toll_cost = Some(toll);

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_err());
        // Inventory unchanged on failure
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(2));
    }

    #[test]
    fn move_with_multi_resource_toll() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        agent.inventory.insert(Resource::Stone, 8);
        let dest = LocationId::new();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(2);
        ctx.move_destination = Some(dest);
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 3);
        toll.insert(Resource::Stone, 2);
        ctx.move_toll_cost = Some(toll);

        let result = execute_move(&mut agent, dest, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(7));
        assert_eq!(agent.inventory.get(&Resource::Stone).copied(), Some(6));
        assert_eq!(hr.outcome.resource_changes.get(&Resource::Wood).copied(), Some(-3));
        assert_eq!(hr.outcome.resource_changes.get(&Resource::Stone).copied(), Some(-2));
    }

    #[test]
    fn dispatch_move_with_toll_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 10);
        let dest = LocationId::new();
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.travel_cost = Some(3);
        ctx.move_destination = Some(dest);
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 4);
        ctx.move_toll_cost = Some(toll);

        let result = execute_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.began_travel);
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(6));
    }

    // -----------------------------------------------------------------------
    // Governance: Claim (Phase 4.4.1)
    // -----------------------------------------------------------------------

    #[test]
    fn claim_unowned_structure_succeeds() {
        let mut agent = make_agent(80);
        let structure = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            None, // unowned
        );
        let sid = structure.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(sid, structure);

        let result = execute_claim(&mut agent, sid, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(hr.structure_claimed, Some(sid));
        assert_eq!(hr.outcome.energy_spent, costs::energy_cost(ActionType::Claim));
        assert_eq!(agent.energy, 80u32.saturating_sub(costs::energy_cost(ActionType::Claim)));
    }

    #[test]
    fn claim_dead_owner_structure_succeeds() {
        let mut agent = make_agent(80);
        let dead_owner = AgentId::new();
        let structure = make_test_structure(
            StructureType::BasicHut,
            agent.location_id,
            Some(dead_owner),
        );
        let sid = structure.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(sid, structure);
        ctx.dead_agents.insert(dead_owner);

        let result = execute_claim(&mut agent, sid, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.structure_claimed, Some(sid));
    }

    #[test]
    fn claim_living_owner_fails() {
        let mut agent = make_agent(80);
        let living_owner = AgentId::new();
        let structure = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            Some(living_owner),
        );
        let sid = structure.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(sid, structure);
        // living_owner is NOT in dead_agents

        let result = execute_claim(&mut agent, sid, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn claim_nonexistent_structure_fails() {
        let mut agent = make_agent(80);
        let ctx = make_exec_ctx();
        let missing_id = StructureId::new();

        let result = execute_claim(&mut agent, missing_id, &ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Governance: Legislate (Phase 4.4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn legislate_creates_rule() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();

        let meeting_hall = make_test_structure(
            StructureType::MeetingHall,
            agent.location_id,
            None,
        );
        let mh_id = meeting_hall.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(mh_id, meeting_hall);
        ctx.agent_groups.insert(group_id);

        let result = execute_legislate(
            &mut agent,
            "No stealing",
            "Agents shall not take others' resources",
            group_id,
            &ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();

        let rule = hr.rule_created.as_ref().unwrap();
        assert_eq!(rule.name, "No stealing");
        assert_eq!(rule.group_id, group_id);
        assert_eq!(rule.creator, agent.agent_id);
        assert!(rule.active);
        assert_eq!(hr.outcome.energy_spent, costs::energy_cost(ActionType::Legislate));
    }

    #[test]
    fn legislate_without_meeting_hall_fails() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();

        let mut ctx = make_exec_ctx();
        ctx.agent_groups.insert(group_id);
        // No MeetingHall at location

        let result = execute_legislate(
            &mut agent,
            "No stealing",
            "Do not steal",
            group_id,
            &ctx,
        );
        assert!(result.is_err());
    }

    #[test]
    fn legislate_without_group_membership_fails() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();

        let meeting_hall = make_test_structure(
            StructureType::MeetingHall,
            agent.location_id,
            None,
        );
        let mh_id = meeting_hall.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(mh_id, meeting_hall);
        // agent_groups is empty -- agent not in the group

        let result = execute_legislate(
            &mut agent,
            "No stealing",
            "Do not steal",
            group_id,
            &ctx,
        );
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Governance: Enforce (Phase 4.4.3)
    // -----------------------------------------------------------------------

    #[test]
    fn enforce_emits_enforcement_details() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();
        let target = AgentId::new();
        let rule_id = RuleId::new();

        let rule = Rule {
            id: rule_id,
            group_id,
            creator: agent.agent_id,
            name: String::from("No stealing"),
            description: String::from("Do not steal"),
            created_at_tick: 0,
            active: true,
        };

        let mut ctx = make_exec_ctx();
        ctx.agent_groups.insert(group_id);
        ctx.active_rules.insert(rule_id, rule);

        let result = execute_enforce(&mut agent, target, rule_id, "Warning issued", &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        let enf = hr.enforcement.as_ref().unwrap();
        assert_eq!(enf.rule_id, rule_id);
        assert_eq!(enf.enforcer, agent.agent_id);
        assert_eq!(enf.target, target);
        assert_eq!(enf.group_id, group_id);
        assert_eq!(enf.consequence, "Warning issued");
        assert_eq!(hr.outcome.energy_spent, costs::energy_cost(ActionType::Enforce));
    }

    #[test]
    fn enforce_without_group_membership_fails() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();
        let target = AgentId::new();
        let rule_id = RuleId::new();

        let rule = Rule {
            id: rule_id,
            group_id,
            creator: AgentId::new(), // someone else created it
            name: String::from("No stealing"),
            description: String::from("Do not steal"),
            created_at_tick: 0,
            active: true,
        };

        let mut ctx = make_exec_ctx();
        // agent_groups is empty -- not a member of the group
        ctx.active_rules.insert(rule_id, rule);

        let result = execute_enforce(&mut agent, target, rule_id, "Warning issued", &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn enforce_nonexistent_rule_fails() {
        let mut agent = make_agent(80);
        let group_id = GroupId::new();
        let target = AgentId::new();
        let rule_id = RuleId::new();

        let mut ctx = make_exec_ctx();
        ctx.agent_groups.insert(group_id);
        // active_rules is empty

        let result = execute_enforce(&mut agent, target, rule_id, "Warning issued", &ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Governance dispatch via execute_action (Phase 4.4.4)
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_claim_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let structure = make_test_structure(
            StructureType::Campfire,
            agent.location_id,
            None,
        );
        let sid = structure.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(sid, structure);

        let result = execute_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert_eq!(hr.structure_claimed, Some(sid));
    }

    #[test]
    fn dispatch_legislate_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let group_id = GroupId::new();

        let meeting_hall = make_test_structure(
            StructureType::MeetingHall,
            agent.location_id,
            None,
        );
        let mh_id = meeting_hall.id;

        let mut ctx = make_exec_ctx();
        ctx.structures_at_location.insert(mh_id, meeting_hall);
        ctx.agent_groups.insert(group_id);

        let result = execute_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("Be kind"),
                rule_description: String::from("Treat others well"),
                group_id,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.rule_created.is_some());
    }

    #[test]
    fn dispatch_enforce_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let group_id = GroupId::new();
        let target = AgentId::new();
        let rule_id = RuleId::new();

        let rule = Rule {
            id: rule_id,
            group_id,
            creator: agent.agent_id,
            name: String::from("Be kind"),
            description: String::from("Treat others well"),
            created_at_tick: 0,
            active: true,
        };

        let mut ctx = make_exec_ctx();
        ctx.agent_groups.insert(group_id);
        ctx.active_rules.insert(rule_id, rule);

        let result = execute_action(
            ActionType::Enforce,
            &ActionParameters::Enforce {
                target_agent: target,
                rule_id,
                consequence: String::from("Verbal warning"),
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.enforcement.is_some());
    }

    // -----------------------------------------------------------------------
    // Farm Plant handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn farm_plant_deducts_seed_and_registers() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::FoodBerry, 3);
        let location = agent.location_id;

        let mut ctx = make_exec_ctx();
        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);

        let result = execute_farm_plant(&mut agent, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Seed consumed
        assert_eq!(agent.inventory.get(&Resource::FoodBerry).copied(), Some(2));
        // Energy deducted
        assert_eq!(hr.outcome.energy_spent, 20);
        // Farm planted signal
        assert!(hr.farm_planted.is_some());
        let (planted_id, _mature_at) = hr.farm_planted.unwrap();
        assert_eq!(planted_id, farm_id);
        // XP awarded
        assert_eq!(
            hr.outcome.skill_xp.get("farming").copied(),
            Some(skills::XP_FARM_PLANT)
        );
    }

    #[test]
    fn farm_plant_no_farm_plot_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_exec_ctx();
        // No FarmPlot in structures_at_location

        let result = execute_farm_plant(&mut agent, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn farm_plant_no_seed_fails() {
        let mut agent = make_agent(80);
        // No food in inventory
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        ctx.structures_at_location.insert(farm.id, farm);

        let result = execute_farm_plant(&mut agent, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn farm_plant_already_planted_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::FoodBerry, 3);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);
        // Pre-plant crops on the farm
        ctx.farm_registry.plant(farm_id, 1, 10);

        let result = execute_farm_plant(&mut agent, &mut ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Farm Harvest handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn farm_harvest_yields_food() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        ctx.current_tick = 20;

        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);
        // Plant at tick 5, mature at tick 15 (growth = 10)
        ctx.farm_registry.plant(farm_id, 5, 10);

        let result = execute_farm_harvest(&mut agent, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Base yield is 5 (no skill bonus)
        assert_eq!(
            agent.inventory.get(&Resource::FoodFarmed).copied(),
            Some(5)
        );
        assert_eq!(hr.outcome.energy_spent, 10);
        assert!(hr.farm_harvested.is_some());
        assert_eq!(hr.farm_harvested.unwrap(), farm_id);
        assert_eq!(
            hr.outcome.skill_xp.get("farming").copied(),
            Some(skills::XP_FARM_HARVEST)
        );
    }

    #[test]
    fn farm_harvest_with_skill_bonus() {
        let mut agent = make_agent(80);
        agent.skills.insert(String::from("farming"), 6);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        ctx.current_tick = 20;

        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);
        ctx.farm_registry.plant(farm_id, 5, 10);

        let result = execute_farm_harvest(&mut agent, &mut ctx);
        assert!(result.is_ok());

        // Yield: 5 + 6/2 = 5 + 3 = 8
        assert_eq!(
            agent.inventory.get(&Resource::FoodFarmed).copied(),
            Some(8)
        );
    }

    #[test]
    fn farm_harvest_immature_crops_fails() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        ctx.current_tick = 10; // Crops planted at 5, mature at 15

        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);
        ctx.farm_registry.plant(farm_id, 5, 10);

        let result = execute_farm_harvest(&mut agent, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn farm_harvest_no_crops_fails() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();
        ctx.current_tick = 20;

        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        ctx.structures_at_location.insert(farm.id, farm);
        // No crops planted

        let result = execute_farm_harvest(&mut agent, &mut ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Craft handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn craft_tool_deducts_inputs_and_adds_output() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 5);
        agent.inventory.insert(Resource::Stone, 4);

        let ctx = make_exec_ctx();
        let result = execute_craft(&mut agent, Resource::Tool, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // 3 wood consumed, 2 stone consumed
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(2));
        assert_eq!(agent.inventory.get(&Resource::Stone).copied(), Some(2));
        // 1 tool produced
        assert_eq!(agent.inventory.get(&Resource::Tool).copied(), Some(1));
        assert_eq!(hr.outcome.energy_spent, 15);
        assert_eq!(
            hr.outcome.skill_xp.get("crafting").copied(),
            Some(skills::XP_CRAFT)
        );
    }

    #[test]
    fn craft_advanced_tool_deducts_metal_and_wood() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Metal, 3);
        agent.inventory.insert(Resource::Wood, 2);

        let ctx = make_exec_ctx();
        let result = execute_craft(&mut agent, Resource::ToolAdvanced, &ctx);
        assert!(result.is_ok());

        // 2 metal consumed, 1 wood consumed
        assert_eq!(agent.inventory.get(&Resource::Metal).copied(), Some(1));
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(1));
        assert_eq!(
            agent.inventory.get(&Resource::ToolAdvanced).copied(),
            Some(1)
        );
    }

    #[test]
    fn craft_medicine_deducts_berries_and_water() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::FoodBerry, 5);
        agent.inventory.insert(Resource::Water, 2);

        let ctx = make_exec_ctx();
        let result = execute_craft(&mut agent, Resource::Medicine, &ctx);
        assert!(result.is_ok());

        assert_eq!(
            agent.inventory.get(&Resource::FoodBerry).copied(),
            Some(2)
        );
        assert_eq!(agent.inventory.get(&Resource::Water).copied(), Some(1));
        assert_eq!(agent.inventory.get(&Resource::Medicine).copied(), Some(1));
    }

    #[test]
    fn craft_insufficient_inputs_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 1); // Need 3
        agent.inventory.insert(Resource::Stone, 2);

        let ctx = make_exec_ctx();
        let result = execute_craft(&mut agent, Resource::Tool, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn craft_invalid_output_fails() {
        let mut agent = make_agent(80);
        let ctx = make_exec_ctx();
        // Wood is not craftable
        let result = execute_craft(&mut agent, Resource::Wood, &ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Mine handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn mine_yields_ore() {
        let mut agent = make_agent(80);
        let mut ctx = make_exec_ctx();
        ctx.location_resources.insert(Resource::Ore, 20);

        let result = execute_mine(&mut agent, &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // Base yield is 2 (no skill bonus)
        assert_eq!(agent.inventory.get(&Resource::Ore).copied(), Some(2));
        assert_eq!(hr.location_resource_deltas.get(&Resource::Ore).copied(), Some(2));
        assert_eq!(hr.outcome.energy_spent, 20);
        assert_eq!(
            hr.outcome.skill_xp.get("mining").copied(),
            Some(skills::XP_MINE)
        );
    }

    #[test]
    fn mine_with_skill_bonus() {
        let mut agent = make_agent(80);
        agent.skills.insert(String::from("mining"), 4);
        let mut ctx = make_exec_ctx();
        ctx.location_resources.insert(Resource::Ore, 20);

        let result = execute_mine(&mut agent, &mut ctx);
        assert!(result.is_ok());

        // Yield: 2 + 4/2 = 2 + 2 = 4
        assert_eq!(agent.inventory.get(&Resource::Ore).copied(), Some(4));
    }

    #[test]
    fn mine_caps_at_available() {
        let mut agent = make_agent(80);
        let mut ctx = make_exec_ctx();
        ctx.location_resources.insert(Resource::Ore, 1); // Only 1 available

        let result = execute_mine(&mut agent, &mut ctx);
        assert!(result.is_ok());

        assert_eq!(agent.inventory.get(&Resource::Ore).copied(), Some(1));
    }

    #[test]
    fn mine_zero_ore_yields_zero() {
        let mut agent = make_agent(80);
        let mut ctx = make_exec_ctx();
        ctx.location_resources.insert(Resource::Ore, 0);

        let result = execute_mine(&mut agent, &mut ctx);
        assert!(result.is_ok());
        // When available ore is 0, actual yield is 0 -- add_resource inserts 0
        assert_eq!(agent.inventory.get(&Resource::Ore).copied().unwrap_or(0), 0);
    }

    // -----------------------------------------------------------------------
    // Smelt handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn smelt_converts_ore_to_metal() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Ore, 4);
        agent.inventory.insert(Resource::Wood, 3);

        let ctx = make_exec_ctx();
        let result = execute_smelt(&mut agent, &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        // 2 ore consumed, 1 wood consumed, 1 metal produced
        assert_eq!(agent.inventory.get(&Resource::Ore).copied(), Some(2));
        assert_eq!(agent.inventory.get(&Resource::Wood).copied(), Some(2));
        assert_eq!(agent.inventory.get(&Resource::Metal).copied(), Some(1));
        assert_eq!(hr.outcome.energy_spent, 20);
        assert_eq!(
            hr.outcome.skill_xp.get("smelting").copied(),
            Some(skills::XP_SMELT)
        );
    }

    #[test]
    fn smelt_insufficient_ore_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Ore, 1); // Need 2
        agent.inventory.insert(Resource::Wood, 3);

        let ctx = make_exec_ctx();
        let result = execute_smelt(&mut agent, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn smelt_insufficient_wood_fails() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Ore, 4);
        // No wood

        let ctx = make_exec_ctx();
        let result = execute_smelt(&mut agent, &ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Write handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn write_stores_knowledge_in_library() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();

        let library = make_test_structure(StructureType::Library, location, Some(agent.agent_id));
        let library_id = library.id;
        ctx.structures_at_location.insert(library_id, library);

        let result = execute_write(&mut agent, "agriculture", &mut ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(hr.outcome.energy_spent, 5);
        assert!(hr.library_write.is_some());
        let (write_lib_id, concept) = hr.library_write.unwrap();
        assert_eq!(write_lib_id, library_id);
        assert_eq!(concept, "agriculture");
        // Knowledge persisted in context
        assert!(ctx
            .library_knowledge
            .get(&library_id)
            .unwrap()
            .contains("agriculture"));
    }

    #[test]
    fn write_no_library_fails() {
        let mut agent = make_agent(80);
        let mut ctx = make_exec_ctx();
        // No library in structures

        let result = execute_write(&mut agent, "agriculture", &mut ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Read handler (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn read_returns_knowledge_from_library() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let mut ctx = make_exec_ctx();

        let library = make_test_structure(StructureType::Library, location, Some(agent.agent_id));
        let library_id = library.id;
        ctx.structures_at_location.insert(library_id, library);
        // Pre-populate library with knowledge
        let mut concepts = BTreeSet::new();
        concepts.insert(String::from("metalworking"));
        ctx.library_knowledge.insert(library_id, concepts);

        let result = execute_read(&mut agent, "metalworking", &ctx);
        assert!(result.is_ok());
        let hr = result.unwrap();

        assert_eq!(hr.outcome.energy_spent, 5);
        assert!(hr.library_read.is_some());
        let (read_lib_id, concept) = hr.library_read.unwrap();
        assert_eq!(read_lib_id, library_id);
        assert_eq!(concept, "metalworking");
    }

    #[test]
    fn read_no_library_fails() {
        let mut agent = make_agent(80);
        let ctx = make_exec_ctx();

        let result = execute_read(&mut agent, "agriculture", &ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Dispatch tests for new actions (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_farm_plant_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::FoodBerry, 3);
        let location = agent.location_id;
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        ctx.structures_at_location.insert(farm.id, farm);

        let result = execute_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.farm_planted.is_some());
    }

    #[test]
    fn dispatch_farm_harvest_via_execute_action() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.current_tick = 20;
        let farm = make_test_structure(StructureType::FarmPlot, location, Some(agent.agent_id));
        let farm_id = farm.id;
        ctx.structures_at_location.insert(farm_id, farm);
        ctx.farm_registry.plant(farm_id, 5, 10);

        let result = execute_action(
            ActionType::FarmHarvest,
            &ActionParameters::FarmHarvest,
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.farm_harvested.is_some());
    }

    #[test]
    fn dispatch_craft_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Wood, 5);
        agent.inventory.insert(Resource::Stone, 4);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Tool,
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        assert_eq!(agent.inventory.get(&Resource::Tool).copied(), Some(1));
    }

    #[test]
    fn dispatch_mine_via_execute_action() {
        let mut agent = make_agent(80);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        ctx.location_resources.insert(Resource::Ore, 20);

        let result = execute_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        assert_eq!(agent.inventory.get(&Resource::Ore).copied(), Some(2));
    }

    #[test]
    fn dispatch_smelt_via_execute_action() {
        let mut agent = make_agent(80);
        agent.inventory.insert(Resource::Ore, 4);
        agent.inventory.insert(Resource::Wood, 3);
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();

        let result = execute_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        assert_eq!(agent.inventory.get(&Resource::Metal).copied(), Some(1));
    }

    #[test]
    fn dispatch_write_via_execute_action() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        let library = make_test_structure(StructureType::Library, location, Some(agent.agent_id));
        ctx.structures_at_location.insert(library.id, library);

        let result = execute_action(
            ActionType::Write,
            &ActionParameters::Write {
                knowledge: String::from("agriculture"),
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.library_write.is_some());
    }

    #[test]
    fn dispatch_read_via_execute_action() {
        let mut agent = make_agent(80);
        let location = agent.location_id;
        let config = VitalsConfig::default();
        let mut ctx = make_exec_ctx();
        let library = make_test_structure(StructureType::Library, location, Some(agent.agent_id));
        let library_id = library.id;
        ctx.structures_at_location.insert(library_id, library);
        let mut concepts = BTreeSet::new();
        concepts.insert(String::from("mining"));
        ctx.library_knowledge.insert(library_id, concepts);

        let result = execute_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("mining"),
            },
            &mut agent,
            &config,
            &mut ctx,
        );
        assert!(result.is_ok());
        let hr = result.unwrap();
        assert!(hr.library_read.is_some());
    }
}
