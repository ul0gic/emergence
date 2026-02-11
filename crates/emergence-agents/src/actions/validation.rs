//! Action validation pipeline per `world-engine.md` section 7.2.
//!
//! The pipeline runs 7 stages in order:
//! 1. Syntax -- is the action well-formed?
//! 2. Vitals -- does the agent have enough energy?
//! 3. Location -- is the agent at the right location?
//! 4. Resources -- does the location/agent have the resource?
//! 5. World state -- any world-level blocks (storms, etc.)?
//! 6. Skill -- does the agent have the knowledge? (stub for Phase 2)
//! 7. Conflict -- reserved for the conflict resolution pass.
//!
//! Each stage returns `Ok(())` on success or a [`RejectionReason`] on failure.

use std::collections::{BTreeMap, BTreeSet};

use emergence_types::{
    ActionParameters, ActionType, AgentId, AgentState, GroupId, LocationId, RejectionReason,
    Resource, ResourceNode, Route, Structure, StructureId, StructureType,
};

use emergence_world::farming;

use crate::crafting;
use crate::reproduction;

use super::costs;

/// Context needed to validate an action against the world state.
///
/// Assembled by the tick cycle from the world map and agent data.
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// The acting agent's ID.
    pub agent_id: emergence_types::AgentId,
    /// The agent's current location ID.
    pub agent_location: LocationId,
    /// Whether the agent is currently traveling (cannot act except wait).
    pub is_traveling: bool,
    /// Resources available at the agent's current location.
    pub location_resources: BTreeMap<Resource, ResourceNode>,
    /// Agent IDs present at the same location.
    pub agents_at_location: Vec<emergence_types::AgentId>,
    /// Whether travel is blocked by weather (storms).
    pub travel_blocked: bool,
    /// The agent's knowledge set (for skill checks).
    pub agent_knowledge: std::collections::BTreeSet<String>,
    /// Whether the agent is mature (has lived at least `maturity_ticks`).
    ///
    /// Computed by the caller from the agent's age. Immature agents are
    /// restricted from advanced actions (build, trade, teach, reproduce, etc.).
    pub is_mature: bool,
    /// Structures at the agent's current location, keyed by structure ID.
    ///
    /// Used for repair, demolish, and build validation. The caller populates
    /// this from the world state.
    pub structures_at_location: BTreeMap<StructureId, Structure>,
    /// The route being targeted by an `ImproveRoute` action, if any.
    ///
    /// Populated by the caller from the world map when the action is
    /// `ImproveRoute`. Contains the route connecting the agent's location
    /// to the destination specified in the action parameters.
    pub route_to_improve: Option<Route>,
    /// The route being used for a `Move` action, if any.
    ///
    /// Populated by the caller from the world map when the action is `Move`.
    /// Used for ACL checks (access control and toll costs) during validation.
    pub move_route: Option<Route>,
    /// The agent's group memberships, used for ACL group-based access checks.
    ///
    /// Populated by the caller from the agent's social graph. An empty list
    /// means the agent belongs to no groups.
    pub agent_groups: Vec<GroupId>,
    /// Set of agent IDs known to be dead.
    ///
    /// Used by the `Claim` validation to determine if a structure's current
    /// owner has died, making the structure eligible for claiming.
    /// Populated by the tick cycle from the agent manager.
    pub dead_agents: BTreeSet<AgentId>,
    /// The farm registry tracking crop growth state on farm plots.
    ///
    /// Populated by the tick cycle. Used by `FarmPlant` and `FarmHarvest`
    /// location validation to check whether a plot is available or harvestable.
    pub farm_registry: farming::FarmRegistry,
    /// Knowledge concepts stored in library structures at this location.
    ///
    /// Populated by the tick cycle from library state in Dragonfly.
    /// Used by `Read` validation to check if the requested concept exists.
    pub library_knowledge: BTreeMap<StructureId, BTreeSet<String>>,
    /// The current tick number.
    ///
    /// Needed by farm harvest validation to check crop maturity.
    pub current_tick: u64,
}

/// Validate an action through the full pipeline.
///
/// Returns `Ok(())` if the action passes all checks, or a [`RejectionReason`]
/// describing why it was rejected.
///
/// The conflict stage (7) is handled separately by the conflict resolution
/// system, so this pipeline covers stages 1--6.
pub fn validate_action(
    action_type: ActionType,
    params: &ActionParameters,
    agent_state: &AgentState,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    // Stage 1: Syntax check
    validate_syntax(action_type, params)?;

    // NoAction always passes
    if action_type == ActionType::NoAction {
        return Ok(());
    }

    // Traveling agents can only do NoAction
    if context.is_traveling {
        return Err(RejectionReason::WrongLocation);
    }

    // Stage 2: Vitals check
    validate_vitals(action_type, agent_state)?;

    // Maturity gate: reject restricted actions for immature agents
    validate_maturity(action_type, context)?;

    // Stage 3: Location check
    validate_location(action_type, params, context)?;

    // Stage 4: Resource check
    validate_resources(action_type, params, agent_state, context)?;

    // Stage 5: World state check
    validate_world_state(action_type, params, context)?;

    // Stage 6: Skill / knowledge check
    validate_skill(action_type, params, context)?;

    Ok(())
}

/// Stage 1: Syntax validation -- is the action well-formed?
///
/// Checks that the action type matches the parameters variant.
const fn validate_syntax(
    action_type: ActionType,
    params: &ActionParameters,
) -> Result<(), RejectionReason> {
    let valid = matches!(
        (action_type, params),
        (ActionType::Gather, ActionParameters::Gather { .. })
            | (ActionType::Eat, ActionParameters::Eat { .. })
            | (ActionType::Drink, ActionParameters::Drink)
            | (ActionType::Rest, ActionParameters::Rest)
            | (ActionType::Move, ActionParameters::Move { .. })
            | (ActionType::Build, ActionParameters::Build { .. })
            | (ActionType::Repair, ActionParameters::Repair { .. })
            | (ActionType::Demolish, ActionParameters::Demolish { .. })
            | (ActionType::ImproveRoute, ActionParameters::ImproveRoute { .. })
            | (ActionType::Communicate, ActionParameters::Communicate { .. })
            | (ActionType::Broadcast, ActionParameters::Broadcast { .. })
            | (ActionType::TradeOffer, ActionParameters::TradeOffer { .. })
            | (ActionType::TradeAccept, ActionParameters::TradeAccept { .. })
            | (ActionType::TradeReject, ActionParameters::TradeReject { .. })
            | (ActionType::FormGroup, ActionParameters::FormGroup { .. })
            | (ActionType::Teach, ActionParameters::Teach { .. })
            | (ActionType::FarmPlant, ActionParameters::FarmPlant)
            | (ActionType::FarmHarvest, ActionParameters::FarmHarvest)
            | (ActionType::Craft, ActionParameters::Craft { .. })
            | (ActionType::Mine, ActionParameters::Mine)
            | (ActionType::Smelt, ActionParameters::Smelt)
            | (ActionType::Write, ActionParameters::Write { .. })
            | (ActionType::Read, ActionParameters::Read { .. })
            | (ActionType::Claim, ActionParameters::Claim { .. })
            | (ActionType::Legislate, ActionParameters::Legislate { .. })
            | (ActionType::Enforce, ActionParameters::Enforce { .. })
            | (ActionType::Reproduce, ActionParameters::Reproduce { .. })
            | (ActionType::Steal, ActionParameters::Steal { .. })
            | (ActionType::Attack, ActionParameters::Attack { .. })
            | (ActionType::Intimidate, ActionParameters::Intimidate { .. })
            | (ActionType::Propose, ActionParameters::Propose { .. })
            | (ActionType::Vote, ActionParameters::Vote { .. })
            | (ActionType::Marry, ActionParameters::Marry { .. })
            | (ActionType::Divorce, ActionParameters::Divorce { .. })
            | (ActionType::Conspire, ActionParameters::Conspire { .. })
            | (ActionType::Pray, ActionParameters::Pray { .. })
            | (ActionType::Freeform, ActionParameters::Freeform(_))
            | (ActionType::NoAction, ActionParameters::NoAction)
    );

    if valid { Ok(()) } else { Err(RejectionReason::InvalidAction) }
}

/// Stage 2: Vitals check -- does the agent have enough energy?
const fn validate_vitals(
    action_type: ActionType,
    agent_state: &AgentState,
) -> Result<(), RejectionReason> {
    let cost = costs::energy_cost(action_type);
    if agent_state.energy < cost {
        Err(RejectionReason::InsufficientEnergy)
    } else {
        Ok(())
    }
}

/// Maturity gate: reject restricted actions for immature agents.
///
/// Immature agents (less than `maturity_ticks` old) cannot perform advanced
/// actions like build, trade, teach, or reproduce. They can only gather (at
/// reduced yield), eat, drink, rest, move, and communicate.
const fn validate_maturity(
    action_type: ActionType,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    if !context.is_mature && reproduction::is_action_restricted_for_immature(action_type) {
        return Err(RejectionReason::InvalidAction);
    }
    Ok(())
}

/// Stage 3: Location check -- is the agent at the right location?
///
/// For gather: the resource must exist at the location.
/// For move: a route must exist (checked later in world state).
/// For eat/drink: the resource must be in inventory or at location.
/// For communicate: target agent must be at the same location.
/// For claim: structure must exist at location with no living owner.
/// For legislate: agent must be in group, `MeetingHall` at location.
/// For enforce: target agent must be at the same location.
#[allow(clippy::too_many_lines)]
fn validate_location(
    action_type: ActionType,
    params: &ActionParameters,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    match (action_type, params) {
        (ActionType::Gather, ActionParameters::Gather { resource }) => {
            // Resource must exist at location
            if !context.location_resources.contains_key(resource) {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Move, ActionParameters::Move { .. }) => {
            // Check route ACL: the agent must be permitted to traverse the route.
            // If no route is provided in context, the world state check (stage 5)
            // will catch the missing route via travel_blocked or the execution
            // layer via missing travel_cost.
            if let Some(route) = &context.move_route
                && !emergence_world::route::can_traverse(
                    route,
                    context.agent_id,
                    &context.agent_groups,
                )
            {
                return Err(RejectionReason::PermissionDenied);
            }
        }
        (ActionType::Communicate, ActionParameters::Communicate { target_agent, message }) => {
            // Target agent must be present at the same location
            if !context.agents_at_location.contains(target_agent) {
                return Err(RejectionReason::InvalidTarget);
            }
            // Message must not be empty
            if message.is_empty() {
                return Err(RejectionReason::InvalidAction);
            }
        }
        (ActionType::Broadcast, ActionParameters::Broadcast { message }) => {
            // Message must not be empty
            if message.is_empty() {
                return Err(RejectionReason::InvalidAction);
            }
        }
        (ActionType::TradeOffer, ActionParameters::TradeOffer { target_agent, .. })
        | (ActionType::Enforce, ActionParameters::Enforce { target_agent, .. })
        | (ActionType::Steal, ActionParameters::Steal { target_agent, .. })
        | (ActionType::Attack, ActionParameters::Attack { target_agent, .. })
        | (ActionType::Intimidate, ActionParameters::Intimidate { target_agent, .. }) => {
            // Target agent must be at the same location
            if !context.agents_at_location.contains(target_agent) {
                return Err(RejectionReason::InvalidTarget);
            }
        }
        (ActionType::Reproduce, ActionParameters::Reproduce { partner_agent }) => {
            // Partner agent must be at the same location
            if !context.agents_at_location.contains(partner_agent) {
                return Err(RejectionReason::InvalidTarget);
            }
        }
        (ActionType::Repair, ActionParameters::Repair { structure_id }) => {
            // Structure must exist at the agent's location
            if !context.structures_at_location.contains_key(structure_id) {
                return Err(RejectionReason::InvalidTarget);
            }
        }
        (ActionType::Demolish, ActionParameters::Demolish { structure_id }) => {
            // Structure must exist at the agent's location
            let structure = context.structures_at_location.get(structure_id);
            match structure {
                None => return Err(RejectionReason::InvalidTarget),
                Some(s) => {
                    // Agent must own the structure or structure must be unowned
                    let is_owner = s.owner.is_some_and(|owner| owner == context.agent_id);
                    let is_unowned = s.owner.is_none();
                    if !is_owner && !is_unowned {
                        return Err(RejectionReason::PermissionDenied);
                    }
                }
            }
        }
        (ActionType::ImproveRoute, ActionParameters::ImproveRoute { .. }) => {
            // A route must exist and the agent must be at one of its endpoints.
            // The caller provides the resolved route in the context.
            let route = context.route_to_improve.as_ref();
            match route {
                None => return Err(RejectionReason::InvalidTarget),
                Some(r) => {
                    if !emergence_world::route::agent_at_route_endpoint(r, context.agent_location) {
                        return Err(RejectionReason::WrongLocation);
                    }
                }
            }
        }
        (ActionType::Claim, ActionParameters::Claim { structure_id }) => {
            // Structure must exist at the agent's location
            let structure = context.structures_at_location.get(structure_id);
            match structure {
                None => return Err(RejectionReason::InvalidTarget),
                Some(s) => {
                    // Structure must be unowned or owner must be dead
                    if let Some(owner) = s.owner
                        && !context.dead_agents.contains(&owner)
                    {
                        return Err(RejectionReason::PermissionDenied);
                    }
                }
            }
        }
        (ActionType::Legislate, ActionParameters::Legislate { group_id, .. }) => {
            // Agent must be a member of the group
            if !context.agent_groups.contains(group_id) {
                return Err(RejectionReason::PermissionDenied);
            }
            // A MeetingHall must exist at the location
            let has_meeting_hall = context
                .structures_at_location
                .values()
                .any(|s| s.structure_type == StructureType::MeetingHall);
            if !has_meeting_hall {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::FarmPlant, ActionParameters::FarmPlant) => {
            // A FarmPlot without crops must exist at the location
            let has_available_plot = context
                .structures_at_location
                .iter()
                .any(|(sid, s)| {
                    s.structure_type == StructureType::FarmPlot
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                        && !context.farm_registry.has_crops(*sid)
                });
            if !has_available_plot {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::FarmHarvest, ActionParameters::FarmHarvest) => {
            // A FarmPlot with mature crops must exist at the location
            let has_harvestable = context
                .structures_at_location
                .iter()
                .any(|(sid, s)| {
                    s.structure_type == StructureType::FarmPlot
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                        && context.farm_registry.is_harvestable(*sid, context.current_tick)
                });
            if !has_harvestable {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Craft, ActionParameters::Craft { .. }) => {
            // A Workshop must exist at the location
            let has_workshop = context
                .structures_at_location
                .values()
                .any(|s| {
                    s.structure_type == StructureType::Workshop
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                });
            if !has_workshop {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Mine, ActionParameters::Mine) => {
            // Ore resource must exist at the location
            if !context.location_resources.contains_key(&Resource::Ore) {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Smelt, ActionParameters::Smelt) => {
            // A Forge must exist at the location
            let has_forge = context
                .structures_at_location
                .values()
                .any(|s| {
                    s.structure_type == StructureType::Forge
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                });
            if !has_forge {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Write, ActionParameters::Write { .. }) => {
            // A Library must exist at the location
            let has_library = context
                .structures_at_location
                .values()
                .any(|s| {
                    s.structure_type == StructureType::Library
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                });
            if !has_library {
                return Err(RejectionReason::WrongLocation);
            }
        }
        (ActionType::Read, ActionParameters::Read { knowledge }) => {
            // A Library must exist at the location that contains the requested knowledge
            let has_readable_library = context
                .structures_at_location
                .iter()
                .any(|(sid, s)| {
                    s.structure_type == StructureType::Library
                        && s.durability > 0
                        && s.destroyed_at_tick.is_none()
                        && context
                            .library_knowledge
                            .get(sid)
                            .is_some_and(|concepts| concepts.contains(knowledge))
                });
            if !has_readable_library {
                return Err(RejectionReason::WrongLocation);
            }
        }
        _ => {
            // Most actions don't have specific location requirements
            // beyond being at *some* location (which is always true if not traveling).
            // Drink's water check is handled in stage 4 (resource check).
        }
    }
    Ok(())
}

/// Stage 4: Resource check -- does the agent/location have the required resources?
#[allow(clippy::too_many_lines)]
fn validate_resources(
    action_type: ActionType,
    params: &ActionParameters,
    agent_state: &AgentState,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    match (action_type, params) {
        (ActionType::Gather, ActionParameters::Gather { resource }) => {
            // The resource node at the location must have available > 0
            if let Some(node) = context.location_resources.get(resource) {
                if node.available == 0 {
                    return Err(RejectionReason::UnavailableTarget);
                }
            } else {
                return Err(RejectionReason::UnavailableTarget);
            }
        }
        (ActionType::Eat, ActionParameters::Eat { food_type }) => {
            // Agent must have the food in inventory
            if !costs::is_food(*food_type) {
                return Err(RejectionReason::InvalidAction);
            }
            let held = agent_state.inventory.get(food_type).copied().unwrap_or(0);
            if held == 0 {
                return Err(RejectionReason::InsufficientResources);
            }
        }
        (ActionType::Drink, ActionParameters::Drink) => {
            // Agent must have water in inventory or water at location
            let in_inventory = agent_state
                .inventory
                .get(&Resource::Water)
                .copied()
                .unwrap_or(0);
            let at_location = context
                .location_resources
                .get(&Resource::Water)
                .map_or(0, |n| n.available);
            if in_inventory == 0 && at_location == 0 {
                return Err(RejectionReason::InsufficientResources);
            }
        }
        (ActionType::Move, ActionParameters::Move { .. }) => {
            // If the route has a toll cost, the agent must have enough
            // resources in inventory to pay the toll.
            if let Some(route) = &context.move_route
                && let Some(toll) = emergence_world::route::toll_cost(route)
            {
                for (resource, &required) in toll {
                    let held = agent_state
                        .inventory
                        .get(resource)
                        .copied()
                        .unwrap_or(0);
                    if held < required {
                        return Err(RejectionReason::InsufficientResources);
                    }
                }
            }
        }
        (ActionType::Build, ActionParameters::Build { structure_type }) => {
            // Agent must have all required materials
            let bp = emergence_world::blueprint(*structure_type);
            for (resource, &required) in &bp.material_costs {
                let held = agent_state.inventory.get(resource).copied().unwrap_or(0);
                if held < required {
                    return Err(RejectionReason::InsufficientResources);
                }
            }
        }
        (ActionType::Repair, ActionParameters::Repair { structure_id }) => {
            // Compute repair cost and check agent has materials
            if let Some(structure) = context.structures_at_location.get(structure_id) {
                let cost = emergence_world::compute_repair_cost(
                    &structure.materials_used,
                    structure.durability,
                    structure.max_durability,
                );
                if let Ok(materials_needed) = cost {
                    for (resource, &required) in &materials_needed {
                        let held = agent_state
                            .inventory
                            .get(resource)
                            .copied()
                            .unwrap_or(0);
                        if held < required {
                            return Err(RejectionReason::InsufficientResources);
                        }
                    }
                }
            }
        }
        (ActionType::ImproveRoute, ActionParameters::ImproveRoute { .. }) => {
            // Agent must have the materials needed for the upgrade.
            // If the route is already at max level, reject.
            if let Some(r) = &context.route_to_improve {
                let next = emergence_world::route::next_path_upgrade(r.path_type);
                // If already at max and durability is full, nothing to do
                if next.is_none() && r.durability == r.max_durability {
                    return Err(RejectionReason::UnavailableTarget);
                }
                // If upgrading (not just repairing), check material costs
                if let Some(target) = next
                    && let Some(costs) = emergence_world::route::upgrade_cost(target)
                {
                    for (resource, &required) in &costs {
                        let held = agent_state
                            .inventory
                            .get(resource)
                            .copied()
                            .unwrap_or(0);
                        if held < required {
                            return Err(RejectionReason::InsufficientResources);
                        }
                    }
                }
                // If repairing (at max level or choosing to repair), no material cost
            }
        }
        (ActionType::TradeOffer, ActionParameters::TradeOffer { offer, .. }) => {
            // Offerer must have all offered resources in inventory
            for (resource, &quantity) in offer {
                let held = agent_state.inventory.get(resource).copied().unwrap_or(0);
                if held < quantity {
                    return Err(RejectionReason::InsufficientResources);
                }
            }
            // Offer map must not be empty
            if offer.is_empty() {
                return Err(RejectionReason::InvalidAction);
            }
        }
        (ActionType::FarmPlant, ActionParameters::FarmPlant) => {
            // Agent must have at least 1 food item as seed
            let has_seed = [
                Resource::FoodBerry,
                Resource::FoodRoot,
                Resource::FoodFish,
                Resource::FoodMeat,
                Resource::FoodFarmed,
                Resource::FoodCooked,
            ]
            .iter()
            .any(|f| agent_state.inventory.get(f).copied().unwrap_or(0) > 0);
            if !has_seed {
                return Err(RejectionReason::InsufficientResources);
            }
        }
        (ActionType::Craft, ActionParameters::Craft { output }) => {
            // Agent must have all recipe inputs
            if let Some(recipe) = crafting::recipe_for(*output) {
                for (resource, &required) in &recipe.inputs {
                    let held = agent_state
                        .inventory
                        .get(resource)
                        .copied()
                        .unwrap_or(0);
                    if held < required {
                        return Err(RejectionReason::InsufficientResources);
                    }
                }
            } else {
                // No recipe exists for this output -- invalid craft target
                return Err(RejectionReason::InvalidAction);
            }
        }
        (ActionType::Mine, ActionParameters::Mine) => {
            // Ore must be available (quantity > 0) at the location
            if let Some(node) = context.location_resources.get(&Resource::Ore) {
                if node.available == 0 {
                    return Err(RejectionReason::UnavailableTarget);
                }
            } else {
                return Err(RejectionReason::UnavailableTarget);
            }
            // Agent must have a Tool in inventory
            let has_tool = agent_state
                .inventory
                .get(&Resource::Tool)
                .copied()
                .unwrap_or(0)
                > 0;
            if !has_tool {
                return Err(RejectionReason::InsufficientResources);
            }
        }
        (ActionType::Smelt, ActionParameters::Smelt) => {
            // Agent must have 2 Ore + 1 Wood
            let ore_held = agent_state
                .inventory
                .get(&Resource::Ore)
                .copied()
                .unwrap_or(0);
            if ore_held < costs::SMELT_ORE_INPUT {
                return Err(RejectionReason::InsufficientResources);
            }
            let wood_held = agent_state
                .inventory
                .get(&Resource::Wood)
                .copied()
                .unwrap_or(0);
            if wood_held < costs::SMELT_WOOD_INPUT {
                return Err(RejectionReason::InsufficientResources);
            }
        }
        _ => {
            // Other actions have resource checks handled in their handlers
        }
    }
    Ok(())
}

/// Stage 5: World state check -- any world-level blocks?
fn validate_world_state(
    action_type: ActionType,
    _params: &ActionParameters,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    // Storm blocks travel
    if action_type == ActionType::Move && context.travel_blocked {
        return Err(RejectionReason::UnavailableTarget);
    }
    Ok(())
}

/// Stage 6: Skill check -- does the agent have the knowledge?
///
/// Survival actions (gather, eat, drink, rest, move) are always available.
/// The teach action requires the teacher to know the concept.
/// Construction actions require specific knowledge per structure type.
fn validate_skill(
    action_type: ActionType,
    params: &ActionParameters,
    context: &ValidationContext,
) -> Result<(), RejectionReason> {
    match (action_type, params) {
        (ActionType::Teach, ActionParameters::Teach { target_agent, knowledge }) => {
            // Teacher must know the concept
            if !context.agent_knowledge.contains(knowledge) {
                return Err(RejectionReason::UnknownAction);
            }
            // Target must be at the same location
            if !context.agents_at_location.contains(target_agent) {
                return Err(RejectionReason::InvalidTarget);
            }
            Ok(())
        }
        (ActionType::Build, ActionParameters::Build { structure_type }) => {
            // Agent must have the required knowledge for this structure type
            let bp = emergence_world::blueprint(*structure_type);
            if !context.agent_knowledge.contains(&bp.required_knowledge) {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::ImproveRoute, ActionParameters::ImproveRoute { .. }) => {
            // Check knowledge requirements for the target path type
            if let Some(r) = &context.route_to_improve
                && let Some(target) = emergence_world::route::next_path_upgrade(r.path_type)
                && !emergence_world::route::has_required_knowledge(
                    target,
                    &context.agent_knowledge,
                )
            {
                return Err(RejectionReason::UnknownAction);
            }
            // Repair (no upgrade) does not require special knowledge
            Ok(())
        }
        (ActionType::Legislate, ActionParameters::Legislate { .. }) => {
            // Legislate requires "governance" or "legislation" knowledge
            if !context.agent_knowledge.contains("governance")
                && !context.agent_knowledge.contains("legislation")
            {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::FarmPlant | ActionType::FarmHarvest, _) => {
            // Farming requires "agriculture" knowledge
            if !context.agent_knowledge.contains("agriculture") {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::Craft, ActionParameters::Craft { output }) => {
            // Crafting requires recipe-specific knowledge
            if let Some(recipe) = crafting::recipe_for(*output)
                && !context.agent_knowledge.contains(recipe.required_knowledge)
            {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::Mine, ActionParameters::Mine) => {
            // Mining requires "mining" knowledge
            if !context.agent_knowledge.contains("mining") {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::Smelt, ActionParameters::Smelt) => {
            // Smelting requires "smelting" or "metalworking" knowledge
            if !context.agent_knowledge.contains("smelting")
                && !context.agent_knowledge.contains("metalworking")
            {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        (ActionType::Write | ActionType::Read, _) => {
            // Reading/writing requires "written_language" knowledge
            if !context.agent_knowledge.contains("written_language") {
                return Err(RejectionReason::UnknownAction);
            }
            Ok(())
        }
        _ => {
            // Other actions pass skill check in this phase
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_types::{
        AccessControlList, AgentId, GroupId, LocationId, PathType, Resource, ResourceNode,
    };

    use super::*;

    fn make_agent_state(energy: u32) -> AgentState {
        AgentState {
            agent_id: AgentId::new(),
            energy,
            health: 100,
            hunger: 0,
            thirst: 0,
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

    fn make_context() -> ValidationContext {
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
        ValidationContext {
            agent_id: AgentId::new(),
            agent_location: LocationId::new(),
            is_traveling: false,
            location_resources: resources,
            agents_at_location: Vec::new(),
            travel_blocked: false,
            agent_knowledge: BTreeSet::new(),
            is_mature: true,
            structures_at_location: BTreeMap::new(),
            route_to_improve: None,
            move_route: None,
            agent_groups: Vec::new(),
            dead_agents: BTreeSet::new(),
            farm_registry: emergence_world::farming::FarmRegistry::new(),
            library_knowledge: BTreeMap::new(),
            current_tick: 0,
        }
    }

    #[test]
    fn syntax_mismatch_rejected() {
        let state = make_agent_state(80);
        let ctx = make_context();
        // Gather action type with Rest parameters
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Rest,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn no_action_always_passes() {
        let state = make_agent_state(0);
        let ctx = make_context();
        let result = validate_action(
            ActionType::NoAction,
            &ActionParameters::NoAction,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn insufficient_energy_rejected() {
        let state = make_agent_state(5); // Gather costs 10
        let ctx = make_context();
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn gather_at_location_without_resource_rejected() {
        let state = make_agent_state(80);
        let ctx = make_context(); // Has Wood and Water, not Stone
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Stone,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn gather_empty_resource_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        // Set wood to 0 available
        if let Some(node) = ctx.location_resources.get_mut(&Resource::Wood) {
            node.available = 0;
        }
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnavailableTarget));
    }

    #[test]
    fn gather_valid_passes() {
        let state = make_agent_state(80);
        let ctx = make_context();
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn eat_without_food_rejected() {
        let state = make_agent_state(80); // empty inventory
        let ctx = make_context();
        let result = validate_action(
            ActionType::Eat,
            &ActionParameters::Eat {
                food_type: Resource::FoodBerry,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn eat_non_food_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let ctx = make_context();
        let result = validate_action(
            ActionType::Eat,
            &ActionParameters::Eat {
                food_type: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn eat_with_food_passes() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::FoodBerry, 5);
        let ctx = make_context();
        let result = validate_action(
            ActionType::Eat,
            &ActionParameters::Eat {
                food_type: Resource::FoodBerry,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn drink_without_water_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.location_resources.remove(&Resource::Water);
        let result = validate_action(
            ActionType::Drink,
            &ActionParameters::Drink,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn drink_from_location_passes() {
        let state = make_agent_state(80);
        let ctx = make_context(); // Has water at location
        let result = validate_action(
            ActionType::Drink,
            &ActionParameters::Drink,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn rest_always_passes() {
        let state = make_agent_state(0);
        let ctx = make_context();
        let result = validate_action(
            ActionType::Rest,
            &ActionParameters::Rest,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn travel_blocked_by_storm() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.travel_blocked = true;
        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnavailableTarget));
    }

    #[test]
    fn traveling_agent_cannot_act() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.is_traveling = true;
        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    // -----------------------------------------------------------------------
    // Communicate validation
    // -----------------------------------------------------------------------

    #[test]
    fn communicate_valid_co_located() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Communicate,
            &ActionParameters::Communicate {
                target_agent: target,
                message: String::from("Hello"),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn communicate_target_not_at_location() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let ctx = make_context(); // No agents at location

        let result = validate_action(
            ActionType::Communicate,
            &ActionParameters::Communicate {
                target_agent: target,
                message: String::from("Hello"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn communicate_empty_message_rejected() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Communicate,
            &ActionParameters::Communicate {
                target_agent: target,
                message: String::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn communicate_insufficient_energy() {
        let state = make_agent_state(1); // 1 energy, needs 2
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Communicate,
            &ActionParameters::Communicate {
                target_agent: target,
                message: String::from("Hello"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    // -----------------------------------------------------------------------
    // Broadcast validation
    // -----------------------------------------------------------------------

    #[test]
    fn broadcast_valid() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::Broadcast,
            &ActionParameters::Broadcast {
                message: String::from("Attention everyone!"),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn broadcast_empty_message_rejected() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::Broadcast,
            &ActionParameters::Broadcast {
                message: String::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn broadcast_insufficient_energy() {
        let state = make_agent_state(3); // 3 energy, needs 5
        let ctx = make_context();

        let result = validate_action(
            ActionType::Broadcast,
            &ActionParameters::Broadcast {
                message: String::from("Help!"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    // -----------------------------------------------------------------------
    // TradeOffer validation
    // -----------------------------------------------------------------------

    #[test]
    fn trade_offer_valid_co_located() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn trade_offer_target_not_at_location() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let target = AgentId::new();
        let ctx = make_context(); // no agents at location

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn trade_offer_insufficient_resources() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 2); // only 2
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5); // wants to offer 5
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn trade_offer_insufficient_energy() {
        let mut state = make_agent_state(1); // 1 energy, needs 2
        state.inventory.insert(Resource::Wood, 10);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn trade_offer_empty_offer_rejected() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let offer = BTreeMap::new(); // empty
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    // -----------------------------------------------------------------------
    // TradeAccept / TradeReject validation (syntax only - no special checks)
    // -----------------------------------------------------------------------

    #[test]
    fn trade_accept_syntax_valid() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::TradeAccept,
            &ActionParameters::TradeAccept {
                trade_id: emergence_types::TradeId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn trade_reject_syntax_valid() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::TradeReject,
            &ActionParameters::TradeReject {
                trade_id: emergence_types::TradeId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Maturity validation
    // -----------------------------------------------------------------------

    #[test]
    fn immature_agent_cannot_trade() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);
        ctx.is_mature = false;

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn immature_agent_can_gather() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.is_mature = false;

        let result = validate_action(
            ActionType::Gather,
            &ActionParameters::Gather {
                resource: Resource::Wood,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn mature_agent_can_trade() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);
        ctx.is_mature = true;

        let mut offer = BTreeMap::new();
        offer.insert(Resource::Wood, 5);
        let mut request = BTreeMap::new();
        request.insert(Resource::Stone, 3);

        let result = validate_action(
            ActionType::TradeOffer,
            &ActionParameters::TradeOffer {
                target_agent: target,
                offer,
                request,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn immature_agent_cannot_reproduce() {
        let state = make_agent_state(80);
        let partner = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(partner);
        ctx.is_mature = false;

        let result = validate_action(
            ActionType::Reproduce,
            &ActionParameters::Reproduce {
                partner_agent: partner,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    // -----------------------------------------------------------------------
    // Reproduce location validation
    // -----------------------------------------------------------------------

    #[test]
    fn reproduce_partner_not_at_location() {
        let state = make_agent_state(80);
        let partner = AgentId::new();
        let ctx = make_context(); // no agents at location

        let result = validate_action(
            ActionType::Reproduce,
            &ActionParameters::Reproduce {
                partner_agent: partner,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn reproduce_partner_at_location_passes() {
        let state = make_agent_state(80);
        let partner = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(partner);

        let result = validate_action(
            ActionType::Reproduce,
            &ActionParameters::Reproduce {
                partner_agent: partner,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Build validation (Phase 4.1)
    // -----------------------------------------------------------------------

    #[test]
    fn build_campfire_with_materials_and_knowledge() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("build_campfire"));

        let result = validate_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: emergence_types::StructureType::Campfire,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn build_campfire_insufficient_materials() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("build_campfire"));

        let result = validate_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: emergence_types::StructureType::Campfire,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn build_campfire_without_knowledge() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let ctx = make_context();

        let result = validate_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: emergence_types::StructureType::Campfire,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn build_insufficient_energy() {
        let mut state = make_agent_state(10);
        state.inventory.insert(Resource::Wood, 10);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("build_campfire"));

        let result = validate_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: emergence_types::StructureType::Campfire,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn build_basic_hut_requires_both_materials() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 20);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("build_hut"));

        let result = validate_action(
            ActionType::Build,
            &ActionParameters::Build {
                structure_type: emergence_types::StructureType::BasicHut,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    // -----------------------------------------------------------------------
    // Repair validation (Phase 4.1)
    // -----------------------------------------------------------------------

    #[test]
    fn repair_structure_not_at_location() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::Repair,
            &ActionParameters::Repair {
                structure_id: emergence_types::StructureId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    // -----------------------------------------------------------------------
    // Demolish validation (Phase 4.1)
    // -----------------------------------------------------------------------

    #[test]
    fn demolish_structure_not_at_location() {
        let state = make_agent_state(80);
        let ctx = make_context();

        let result = validate_action(
            ActionType::Demolish,
            &ActionParameters::Demolish {
                structure_id: emergence_types::StructureId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn demolish_own_structure_passes() {
        let state = make_agent_state(80);
        let agent_id = state.agent_id;
        let mut ctx = make_context();
        ctx.agent_id = agent_id;

        let struct_id = emergence_types::StructureId::new();
        let bp = emergence_world::blueprint(emergence_types::StructureType::Campfire);
        let structure = emergence_types::Structure {
            id: struct_id,
            structure_type: emergence_types::StructureType::Campfire,
            subtype: None,
            location_id: ctx.agent_location,
            builder: agent_id,
            owner: Some(agent_id),
            built_at_tick: 1,
            destroyed_at_tick: None,
            materials_used: bp.material_costs,
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        };
        ctx.structures_at_location.insert(struct_id, structure);

        let result = validate_action(
            ActionType::Demolish,
            &ActionParameters::Demolish {
                structure_id: struct_id,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn demolish_other_agent_structure_denied() {
        let state = make_agent_state(80);
        let mut ctx = make_context();

        let other_agent = AgentId::new();
        let struct_id = emergence_types::StructureId::new();
        let bp = emergence_world::blueprint(emergence_types::StructureType::Campfire);
        let structure = emergence_types::Structure {
            id: struct_id,
            structure_type: emergence_types::StructureType::Campfire,
            subtype: None,
            location_id: ctx.agent_location,
            builder: other_agent,
            owner: Some(other_agent),
            built_at_tick: 1,
            destroyed_at_tick: None,
            materials_used: bp.material_costs,
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        };
        ctx.structures_at_location.insert(struct_id, structure);

        let result = validate_action(
            ActionType::Demolish,
            &ActionParameters::Demolish {
                structure_id: struct_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::PermissionDenied));
    }

    #[test]
    fn demolish_unowned_structure_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();

        let struct_id = emergence_types::StructureId::new();
        let bp = emergence_world::blueprint(emergence_types::StructureType::Campfire);
        let structure = emergence_types::Structure {
            id: struct_id,
            structure_type: emergence_types::StructureType::Campfire,
            subtype: None,
            location_id: ctx.agent_location,
            builder: AgentId::new(),
            owner: None,
            built_at_tick: 1,
            destroyed_at_tick: None,
            materials_used: bp.material_costs,
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        };
        ctx.structures_at_location.insert(struct_id, structure);

        let result = validate_action(
            ActionType::Demolish,
            &ActionParameters::Demolish {
                structure_id: struct_id,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // ImproveRoute validation (Phase 4.3)
    // -----------------------------------------------------------------------

    fn make_test_route(from: LocationId, to: LocationId, path: PathType) -> Route {
        use rust_decimal::Decimal;

        Route {
            id: emergence_types::RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: emergence_world::route::base_cost_for_path_type(path),
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
    fn improve_route_no_route_rejected() {
        let state = make_agent_state(80);
        let ctx = make_context(); // route_to_improve is None

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn improve_route_not_at_endpoint_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        // Route between two OTHER locations, not agent's location
        let route = make_test_route(LocationId::new(), LocationId::new(), PathType::None);
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn improve_route_insufficient_energy_rejected() {
        let state = make_agent_state(10); // ImproveRoute costs 30
        let mut ctx = make_context();
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::None);
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn improve_route_insufficient_materials_rejected() {
        let state = make_agent_state(80); // No wood in inventory
        let mut ctx = make_context();
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::None);
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        // None -> DirtTrail costs 10 wood
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn improve_route_missing_knowledge_for_road_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 50);
        state.inventory.insert(Resource::Stone, 30);
        let mut ctx = make_context();
        // Route is WornPath -> Road requires basic_engineering
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::WornPath);
        ctx.route_to_improve = Some(route);
        // No knowledge

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn improve_route_with_knowledge_passes() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 50);
        state.inventory.insert(Resource::Stone, 30);
        let mut ctx = make_context();
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::WornPath);
        ctx.route_to_improve = Some(route);
        ctx.agent_knowledge.insert(String::from("basic_engineering"));

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn improve_route_dirt_trail_passes_no_knowledge() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let mut ctx = make_context();
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::None);
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn improve_route_already_max_and_full_durability_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let route = make_test_route(ctx.agent_location, LocationId::new(), PathType::Highway);
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnavailableTarget));
    }

    #[test]
    fn improve_route_repair_at_max_level_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let mut route = make_test_route(ctx.agent_location, LocationId::new(), PathType::Highway);
        route.durability = 50; // Damaged, so repair is valid
        ctx.route_to_improve = Some(route);

        let result = validate_action(
            ActionType::ImproveRoute,
            &ActionParameters::ImproveRoute {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Move ACL validation (Phase 4.3.2)
    // -----------------------------------------------------------------------

    fn make_move_route(from: LocationId, to: LocationId, acl: Option<AccessControlList>) -> Route {
        use rust_decimal::Decimal;

        Route {
            id: emergence_types::RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: 3,
            path_type: PathType::WornPath,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl,
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    #[test]
    fn move_no_acl_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let route = make_move_route(ctx.agent_location, dest, None);
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_public_acl_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: None,
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_denied_agent_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut denied = BTreeSet::new();
        denied.insert(ctx.agent_id);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: denied,
            public: false,
            toll_cost: None,
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::PermissionDenied));
    }

    #[test]
    fn move_allowed_agent_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut allowed = BTreeSet::new();
        allowed.insert(ctx.agent_id);
        let acl = AccessControlList {
            allowed_agents: allowed,
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_unknown_agent_denied_by_default() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        // Non-public ACL with no allowed agents -> denied by default
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::PermissionDenied));
    }

    #[test]
    fn move_group_membership_grants_access() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let group = GroupId::new();
        ctx.agent_groups = vec![group];
        let mut allowed_groups = BTreeSet::new();
        allowed_groups.insert(group);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups,
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Move toll cost validation (Phase 4.3.2)
    // -----------------------------------------------------------------------

    #[test]
    fn move_toll_sufficient_resources_passes() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_toll_insufficient_resources_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 2); // only 2, need 5
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn move_toll_no_resources_rejected() {
        let state = make_agent_state(80); // empty inventory
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Stone, 3);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn move_toll_multiple_resources_all_needed() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        state.inventory.insert(Resource::Stone, 5);
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        toll.insert(Resource::Stone, 3);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn move_toll_multiple_resources_one_insufficient() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 10);
        // Missing stone
        let mut ctx = make_context();
        let dest = LocationId::new();
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        toll.insert(Resource::Stone, 3);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_move_route(ctx.agent_location, dest, Some(acl));
        ctx.move_route = Some(route);

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move { destination: dest },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn move_no_route_context_passes_validation() {
        // If no move_route is provided, validation still passes --
        // the world state check or execution layer will handle the missing route.
        let state = make_agent_state(80);
        let ctx = make_context(); // move_route is None

        let result = validate_action(
            ActionType::Move,
            &ActionParameters::Move {
                destination: LocationId::new(),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Governance: Claim validation (Phase 4.4.1)
    // -----------------------------------------------------------------------

    fn make_val_structure(
        st: emergence_types::StructureType,
        location_id: LocationId,
        owner: Option<AgentId>,
    ) -> Structure {
        let bp = emergence_world::blueprint(st);
        Structure {
            id: emergence_types::StructureId::new(),
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
    fn claim_unowned_structure_passes_validation() {
        let state = make_agent_state(80);
        let mut ctx = make_context();

        let structure = make_val_structure(
            emergence_types::StructureType::Campfire,
            ctx.agent_location,
            None,
        );
        let sid = structure.id;
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn claim_structure_not_at_location_rejected() {
        let state = make_agent_state(80);
        let ctx = make_context();
        let missing_id = emergence_types::StructureId::new();

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim {
                structure_id: missing_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn claim_living_owner_denied() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let living_owner = AgentId::new();

        let structure = make_val_structure(
            emergence_types::StructureType::Campfire,
            ctx.agent_location,
            Some(living_owner),
        );
        let sid = structure.id;
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::PermissionDenied));
    }

    #[test]
    fn claim_dead_owner_passes_validation() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let dead_owner = AgentId::new();

        let structure = make_val_structure(
            emergence_types::StructureType::BasicHut,
            ctx.agent_location,
            Some(dead_owner),
        );
        let sid = structure.id;
        ctx.structures_at_location.insert(sid, structure);
        ctx.dead_agents.insert(dead_owner);

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn claim_insufficient_energy_rejected() {
        let state = make_agent_state(2); // Claim costs 5
        let mut ctx = make_context();

        let structure = make_val_structure(
            emergence_types::StructureType::Campfire,
            ctx.agent_location,
            None,
        );
        let sid = structure.id;
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn immature_agent_cannot_claim() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.is_mature = false;

        let structure = make_val_structure(
            emergence_types::StructureType::Campfire,
            ctx.agent_location,
            None,
        );
        let sid = structure.id;
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Claim,
            &ActionParameters::Claim { structure_id: sid },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    // -----------------------------------------------------------------------
    // Governance: Legislate validation (Phase 4.4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn legislate_with_all_requirements_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let group_id = GroupId::new();
        ctx.agent_groups = vec![group_id];
        ctx.agent_knowledge.insert(String::from("governance"));

        let meeting_hall = make_val_structure(
            emergence_types::StructureType::MeetingHall,
            ctx.agent_location,
            None,
        );
        let mh_id = meeting_hall.id;
        ctx.structures_at_location.insert(mh_id, meeting_hall);

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn legislate_without_group_membership_denied() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let group_id = GroupId::new();
        // agent_groups is empty
        ctx.agent_knowledge.insert(String::from("governance"));

        let meeting_hall = make_val_structure(
            emergence_types::StructureType::MeetingHall,
            ctx.agent_location,
            None,
        );
        let mh_id = meeting_hall.id;
        ctx.structures_at_location.insert(mh_id, meeting_hall);

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::PermissionDenied));
    }

    #[test]
    fn legislate_without_meeting_hall_wrong_location() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let group_id = GroupId::new();
        ctx.agent_groups = vec![group_id];
        ctx.agent_knowledge.insert(String::from("legislation"));
        // No MeetingHall in structures_at_location

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn legislate_without_knowledge_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let group_id = GroupId::new();
        ctx.agent_groups = vec![group_id];
        // No governance/legislation knowledge

        let meeting_hall = make_val_structure(
            emergence_types::StructureType::MeetingHall,
            ctx.agent_location,
            None,
        );
        let mh_id = meeting_hall.id;
        ctx.structures_at_location.insert(mh_id, meeting_hall);

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn legislate_with_legislation_knowledge_passes() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        let group_id = GroupId::new();
        ctx.agent_groups = vec![group_id];
        ctx.agent_knowledge.insert(String::from("legislation"));

        let meeting_hall = make_val_structure(
            emergence_types::StructureType::MeetingHall,
            ctx.agent_location,
            None,
        );
        let mh_id = meeting_hall.id;
        ctx.structures_at_location.insert(mh_id, meeting_hall);

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn legislate_insufficient_energy_rejected() {
        let state = make_agent_state(5); // Legislate costs 10
        let mut ctx = make_context();
        let group_id = GroupId::new();
        ctx.agent_groups = vec![group_id];
        ctx.agent_knowledge.insert(String::from("governance"));

        let meeting_hall = make_val_structure(
            emergence_types::StructureType::MeetingHall,
            ctx.agent_location,
            None,
        );
        let mh_id = meeting_hall.id;
        ctx.structures_at_location.insert(mh_id, meeting_hall);

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    // -----------------------------------------------------------------------
    // Governance: Enforce validation (Phase 4.4.3)
    // -----------------------------------------------------------------------

    #[test]
    fn enforce_target_at_location_passes() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Enforce,
            &ActionParameters::Enforce {
                target_agent: target,
                rule_id: emergence_types::RuleId::new(),
                consequence: String::from("Warning"),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn enforce_target_not_at_location_rejected() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let ctx = make_context(); // no agents at location

        let result = validate_action(
            ActionType::Enforce,
            &ActionParameters::Enforce {
                target_agent: target,
                rule_id: emergence_types::RuleId::new(),
                consequence: String::from("Warning"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidTarget));
    }

    #[test]
    fn enforce_insufficient_energy_rejected() {
        let state = make_agent_state(10); // Enforce costs 15
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Enforce,
            &ActionParameters::Enforce {
                target_agent: target,
                rule_id: emergence_types::RuleId::new(),
                consequence: String::from("Warning"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    #[test]
    fn immature_agent_cannot_legislate() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.is_mature = false;
        let group_id = GroupId::new();

        let result = validate_action(
            ActionType::Legislate,
            &ActionParameters::Legislate {
                rule_name: String::from("No theft"),
                rule_description: String::from("Do not steal"),
                group_id,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn immature_agent_cannot_enforce() {
        let state = make_agent_state(80);
        let target = AgentId::new();
        let mut ctx = make_context();
        ctx.is_mature = false;
        ctx.agents_at_location.push(target);

        let result = validate_action(
            ActionType::Enforce,
            &ActionParameters::Enforce {
                target_agent: target,
                rule_id: emergence_types::RuleId::new(),
                consequence: String::from("Warning"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    // -----------------------------------------------------------------------
    // Helper: make a test structure at a given location
    // -----------------------------------------------------------------------

    fn make_test_structure(
        st: StructureType,
        location_id: LocationId,
        owner: Option<AgentId>,
    ) -> (StructureId, Structure) {
        let bp = emergence_world::blueprint(st);
        let id = StructureId::new();
        let structure = Structure {
            id,
            structure_type: st,
            subtype: None,
            location_id,
            builder: owner.unwrap_or_else(AgentId::new),
            owner,
            built_at_tick: 1,
            destroyed_at_tick: None,
            materials_used: bp.material_costs,
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        };
        (id, structure)
    }

    // -----------------------------------------------------------------------
    // FarmPlant validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn farm_plant_valid_with_plot_and_seed() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn farm_plant_no_plot_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        // No FarmPlot at location

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn farm_plant_no_seed_rejected() {
        let state = make_agent_state(80); // Empty inventory
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn farm_plant_without_agriculture_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_context();
        // No "agriculture" knowledge
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn farm_plant_already_planted_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        ctx.farm_registry.plant(sid, 1, 10);

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn farm_plant_insufficient_energy_rejected() {
        let mut state = make_agent_state(10); // FarmPlant costs 20
        state.inventory.insert(Resource::FoodBerry, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::FarmPlant,
            &ActionParameters::FarmPlant,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientEnergy));
    }

    // -----------------------------------------------------------------------
    // FarmHarvest validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn farm_harvest_valid_with_mature_crops() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        ctx.current_tick = 20;
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        ctx.farm_registry.plant(sid, 5, 10); // mature at tick 15

        let result = validate_action(
            ActionType::FarmHarvest,
            &ActionParameters::FarmHarvest,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn farm_harvest_immature_crops_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        ctx.current_tick = 10; // Crops mature at 15
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        ctx.farm_registry.plant(sid, 5, 10);

        let result = validate_action(
            ActionType::FarmHarvest,
            &ActionParameters::FarmHarvest,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn farm_harvest_no_crops_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("agriculture"));
        ctx.current_tick = 20;
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        // No crops planted

        let result = validate_action(
            ActionType::FarmHarvest,
            &ActionParameters::FarmHarvest,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn farm_harvest_without_agriculture_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.current_tick = 20;
        let (sid, structure) = make_test_structure(
            StructureType::FarmPlot,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        ctx.farm_registry.plant(sid, 5, 10);

        let result = validate_action(
            ActionType::FarmHarvest,
            &ActionParameters::FarmHarvest,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    // -----------------------------------------------------------------------
    // Craft validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn craft_tool_valid_with_workshop_and_materials() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 5);
        state.inventory.insert(Resource::Stone, 4);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("basic_tools"));
        let (sid, structure) = make_test_structure(
            StructureType::Workshop,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Tool,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn craft_no_workshop_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 5);
        state.inventory.insert(Resource::Stone, 4);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("basic_tools"));
        // No Workshop at location

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Tool,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn craft_insufficient_materials_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 1); // Need 3
        state.inventory.insert(Resource::Stone, 4);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("basic_tools"));
        let (sid, structure) = make_test_structure(
            StructureType::Workshop,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Tool,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn craft_without_knowledge_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 5);
        state.inventory.insert(Resource::Stone, 4);
        let mut ctx = make_context();
        // No "basic_tools" knowledge
        let (sid, structure) = make_test_structure(
            StructureType::Workshop,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Tool,
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn craft_invalid_output_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Wood, 50);
        let mut ctx = make_context();
        let (sid, structure) = make_test_structure(
            StructureType::Workshop,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::Wood, // Not craftable
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InvalidAction));
    }

    #[test]
    fn craft_advanced_tool_requires_metalworking() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Metal, 3);
        state.inventory.insert(Resource::Wood, 2);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("metalworking"));
        let (sid, structure) = make_test_structure(
            StructureType::Workshop,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Craft,
            &ActionParameters::Craft {
                output: Resource::ToolAdvanced,
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Mine validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn mine_valid_with_ore_and_tool() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Tool, 1);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("mining"));
        ctx.location_resources.insert(
            Resource::Ore,
            ResourceNode {
                resource: Resource::Ore,
                available: 20,
                regen_per_tick: 1,
                max_capacity: 50,
            },
        );

        let result = validate_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn mine_no_ore_at_location_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Tool, 1);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("mining"));
        // No Ore in location_resources

        let result = validate_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn mine_no_tool_rejected() {
        let state = make_agent_state(80); // No tool in inventory
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("mining"));
        ctx.location_resources.insert(
            Resource::Ore,
            ResourceNode {
                resource: Resource::Ore,
                available: 20,
                regen_per_tick: 1,
                max_capacity: 50,
            },
        );

        let result = validate_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn mine_ore_depleted_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Tool, 1);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("mining"));
        ctx.location_resources.insert(
            Resource::Ore,
            ResourceNode {
                resource: Resource::Ore,
                available: 0,
                regen_per_tick: 1,
                max_capacity: 50,
            },
        );

        let result = validate_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnavailableTarget));
    }

    #[test]
    fn mine_without_mining_knowledge_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Tool, 1);
        let mut ctx = make_context();
        // No "mining" knowledge
        ctx.location_resources.insert(
            Resource::Ore,
            ResourceNode {
                resource: Resource::Ore,
                available: 20,
                regen_per_tick: 1,
                max_capacity: 50,
            },
        );

        let result = validate_action(
            ActionType::Mine,
            &ActionParameters::Mine,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    // -----------------------------------------------------------------------
    // Smelt validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn smelt_valid_with_forge_and_materials() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 4);
        state.inventory.insert(Resource::Wood, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("smelting"));
        let (sid, structure) = make_test_structure(
            StructureType::Forge,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn smelt_no_forge_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 4);
        state.inventory.insert(Resource::Wood, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("smelting"));
        // No Forge at location

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn smelt_insufficient_ore_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 1); // Need 2
        state.inventory.insert(Resource::Wood, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("smelting"));
        let (sid, structure) = make_test_structure(
            StructureType::Forge,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn smelt_insufficient_wood_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 4);
        // No wood
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("smelting"));
        let (sid, structure) = make_test_structure(
            StructureType::Forge,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::InsufficientResources));
    }

    #[test]
    fn smelt_with_metalworking_knowledge_passes() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 4);
        state.inventory.insert(Resource::Wood, 3);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("metalworking")); // Alternative knowledge
        let (sid, structure) = make_test_structure(
            StructureType::Forge,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn smelt_without_knowledge_rejected() {
        let mut state = make_agent_state(80);
        state.inventory.insert(Resource::Ore, 4);
        state.inventory.insert(Resource::Wood, 3);
        let mut ctx = make_context();
        // No smelting or metalworking knowledge
        let (sid, structure) = make_test_structure(
            StructureType::Forge,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Smelt,
            &ActionParameters::Smelt,
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    // -----------------------------------------------------------------------
    // Write validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn write_valid_with_library_and_knowledge() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Write,
            &ActionParameters::Write {
                knowledge: String::from("agriculture"),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn write_no_library_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        // No Library at location

        let result = validate_action(
            ActionType::Write,
            &ActionParameters::Write {
                knowledge: String::from("agriculture"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn write_without_written_language_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        // No "written_language" knowledge
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);

        let result = validate_action(
            ActionType::Write,
            &ActionParameters::Write {
                knowledge: String::from("agriculture"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    // -----------------------------------------------------------------------
    // Read validation (Phase 4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn read_valid_with_library_containing_concept() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        let mut concepts = BTreeSet::new();
        concepts.insert(String::from("metalworking"));
        ctx.library_knowledge.insert(sid, concepts);

        let result = validate_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("metalworking"),
            },
            &state,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn read_concept_not_in_library_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        // Library has different concept
        let mut concepts = BTreeSet::new();
        concepts.insert(String::from("agriculture"));
        ctx.library_knowledge.insert(sid, concepts);

        let result = validate_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("metalworking"), // Not in library
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn read_no_library_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        // No Library at location

        let result = validate_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("metalworking"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }

    #[test]
    fn read_without_written_language_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        // No "written_language" knowledge
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        let mut concepts = BTreeSet::new();
        concepts.insert(String::from("metalworking"));
        ctx.library_knowledge.insert(sid, concepts);

        let result = validate_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("metalworking"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::UnknownAction));
    }

    #[test]
    fn read_empty_library_rejected() {
        let state = make_agent_state(80);
        let mut ctx = make_context();
        ctx.agent_knowledge.insert(String::from("written_language"));
        let (sid, structure) = make_test_structure(
            StructureType::Library,
            ctx.agent_location,
            Some(ctx.agent_id),
        );
        ctx.structures_at_location.insert(sid, structure);
        // Library knowledge map is empty (no concepts written)

        let result = validate_action(
            ActionType::Read,
            &ActionParameters::Read {
                knowledge: String::from("metalworking"),
            },
            &state,
            &ctx,
        );
        assert_eq!(result, Err(RejectionReason::WrongLocation));
    }
}
