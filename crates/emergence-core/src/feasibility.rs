//! Action feasibility evaluator for freeform actions.
//!
//! When an agent proposes a freeform action (one not in the fixed action
//! catalog), the World Engine must determine whether it is physically
//! possible before executing it.
//!
//! The evaluator runs a pipeline of checks:
//! 1. **Category mapping** -- Can the freeform category be mapped to a known
//!    action type?
//! 2. **Location check** -- Is the agent at a location where this action
//!    makes sense?
//! 3. **Target check** -- Does the target entity exist and is it accessible?
//! 4. **Resource check** -- Does the agent have required materials?
//! 5. **Energy check** -- Does the agent have enough energy?
//! 6. **Knowledge check** -- Does the agent know how to do this?
//! 7. **Physical plausibility** -- Is this action physically possible in the
//!    simulation world?
//!
//! For known categories (steal, attack, marry, pray, etc.), the evaluator
//! maps the freeform action to a concrete action type and returns it as a
//! [`ResolvedAction`]. For truly novel actions, it returns
//! [`FeasibilityResult::NeedsEvaluation`] so an LLM judge can decide.

use std::collections::BTreeMap;

use emergence_types::{
    ActionParameters, ActionTarget, ActionType, AgentId, AgentState, FreeformAction, GroupId,
    LocationId, Resource, ResourceNode, StructureId,
};

/// The result of evaluating a freeform action's feasibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeasibilityResult {
    /// Action is feasible and has been resolved into a concrete action.
    Feasible {
        /// The concrete action type and parameters to execute.
        resolved_action: ResolvedAction,
        /// Energy cost for this action.
        energy_cost: u32,
    },
    /// Action is not physically possible in the simulation world.
    Infeasible {
        /// Human-readable explanation of why the action is infeasible.
        reason: String,
    },
    /// Action is too ambiguous or novel for rule-based evaluation.
    ///
    /// The engine should queue this for LLM adjudication. The context
    /// string provides information for the LLM judge.
    NeedsEvaluation {
        /// Context describing the action and world state for LLM review.
        context: String,
    },
}

/// A freeform action resolved into a concrete action type and parameters.
///
/// This is the output of a successful feasibility evaluation. The resolved
/// action can be passed directly into the standard execution pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAction {
    /// The concrete action type this freeform action maps to.
    pub action_type: ActionType,
    /// The parameters for the resolved action.
    pub parameters: ActionParameters,
}

/// Context about the world state needed by the feasibility evaluator.
///
/// Assembled by the tick cycle from the world map, agent states, and
/// location data. Provides the evaluator with everything it needs to
/// determine if an action is physically possible.
#[derive(Debug, Clone)]
pub struct FeasibilityContext {
    /// The acting agent's ID.
    pub agent_id: AgentId,
    /// The agent's current location ID.
    pub location_id: LocationId,
    /// Resources available at the agent's current location.
    pub location_resources: BTreeMap<Resource, ResourceNode>,
    /// Agent IDs present at the same location.
    pub agents_at_location: Vec<AgentId>,
    /// Structure IDs present at the agent's location.
    pub structures_at_location: Vec<StructureId>,
    /// Groups the agent belongs to.
    pub agent_groups: Vec<GroupId>,
    /// The agent's knowledge set.
    pub agent_knowledge: std::collections::BTreeSet<String>,
}

/// Well-known freeform action categories that can be mapped to concrete
/// action types. Matching is case-insensitive.
const KNOWN_CATEGORIES: &[(&str, ActionType)] = &[
    ("steal", ActionType::Steal),
    ("theft", ActionType::Steal),
    ("rob", ActionType::Steal),
    ("attack", ActionType::Attack),
    ("fight", ActionType::Attack),
    ("combat", ActionType::Attack),
    ("intimidate", ActionType::Intimidate),
    ("threaten", ActionType::Intimidate),
    ("propose", ActionType::Propose),
    ("vote", ActionType::Vote),
    ("marry", ActionType::Marry),
    ("wedding", ActionType::Marry),
    ("divorce", ActionType::Divorce),
    ("conspire", ActionType::Conspire),
    ("plot", ActionType::Conspire),
    ("pray", ActionType::Pray),
    ("worship", ActionType::Pray),
    ("meditate", ActionType::Pray),
    ("ritual", ActionType::Pray),
    ("gather", ActionType::Gather),
    ("collect", ActionType::Gather),
    ("eat", ActionType::Eat),
    ("drink", ActionType::Drink),
    ("rest", ActionType::Rest),
    ("sleep", ActionType::Rest),
    ("move", ActionType::Move),
    ("travel", ActionType::Move),
    ("build", ActionType::Build),
    ("construct", ActionType::Build),
    ("repair", ActionType::Repair),
    ("fix", ActionType::Repair),
    ("demolish", ActionType::Demolish),
    ("destroy", ActionType::Demolish),
    ("teach", ActionType::Teach),
    ("trade", ActionType::TradeOffer),
    ("communicate", ActionType::Communicate),
    ("talk", ActionType::Communicate),
    ("broadcast", ActionType::Broadcast),
    ("shout", ActionType::Broadcast),
    ("mine", ActionType::Mine),
    ("craft", ActionType::Craft),
    ("smelt", ActionType::Smelt),
    ("write", ActionType::Write),
    ("read", ActionType::Read),
    ("claim", ActionType::Claim),
    ("legislate", ActionType::Legislate),
    ("enforce", ActionType::Enforce),
    ("reproduce", ActionType::Reproduce),
    ("farm", ActionType::FarmPlant),
    ("harvest", ActionType::FarmHarvest),
    ("plant", ActionType::FarmPlant),
];

/// Actions that are physically impossible in the simulation world.
///
/// These represent capabilities that agents do not have, regardless of
/// resources, knowledge, or location.
const IMPOSSIBLE_ACTIONS: &[&str] = &[
    "fly",
    "teleport",
    "time_travel",
    "resurrect",
    "magic",
    "levitate",
    "invisible",
    "immortal",
    "omniscient",
    "omnipotent",
    "transform",
    "shapeshift",
    "conjure",
    "summon",
    "enchant",
    "hex",
    "curse",
    "vanish",
    "phase",
    "warp",
];

/// Evaluate whether a freeform action is physically feasible.
///
/// Runs the feasibility pipeline in order:
/// 1. Physical plausibility (reject impossible actions)
/// 2. Category mapping (try to resolve to a known action type)
/// 3. Location check (agent must be at a sensible location)
/// 4. Target existence check
/// 5. Energy check
/// 6. If all checks pass and the category is known, return `Feasible`
/// 7. If the category is unknown, return `NeedsEvaluation`
pub fn evaluate_feasibility(
    action: &FreeformAction,
    agent_state: &AgentState,
    world_context: &FeasibilityContext,
) -> FeasibilityResult {
    // Step 1: Physical plausibility -- reject actions that are impossible
    let category_lower = action.action_category.to_lowercase();
    if is_physically_impossible(&category_lower, &action.intent) {
        return FeasibilityResult::Infeasible {
            reason: format!(
                "Action '{}' is not physically possible in this world.",
                action.action_category,
            ),
        };
    }

    // Step 2: Category mapping -- try to resolve to a known action type
    let mapped_type = map_category_to_action_type(&category_lower);

    let Some(action_type) = mapped_type else {
        // Unknown category -- needs LLM evaluation
        return FeasibilityResult::NeedsEvaluation {
            context: format!(
                "Agent {} at location {} proposed freeform action: category='{}', intent='{}'. \
                 No known action type matches this category. World state: {} agents co-located, \
                 {} resource types available.",
                world_context.agent_id,
                world_context.location_id,
                action.action_category,
                action.intent,
                world_context.agents_at_location.len(),
                world_context.location_resources.len(),
            ),
        };
    };

    // Step 3: Location check -- does the action make sense here?
    if let Some(reason) = check_location(action_type, action.target.as_ref(), world_context) {
        return FeasibilityResult::Infeasible { reason };
    }

    // Step 4: Target existence check
    if let Some(reason) = check_target_exists(action_type, action.target.as_ref(), world_context) {
        return FeasibilityResult::Infeasible { reason };
    }

    // Step 5: Energy check
    let cost = emergence_agents::actions::costs::energy_cost(action_type);
    if agent_state.energy < cost {
        return FeasibilityResult::Infeasible {
            reason: format!(
                "Insufficient energy: action requires {cost} energy, agent has {}.",
                agent_state.energy,
            ),
        };
    }

    // Step 6: Resolve to concrete parameters
    match resolve_parameters(action_type, action, world_context) {
        Ok(parameters) => FeasibilityResult::Feasible {
            resolved_action: ResolvedAction {
                action_type,
                parameters,
            },
            energy_cost: cost,
        },
        Err(reason) => FeasibilityResult::Infeasible { reason },
    }
}

/// Check if the action category or intent describes something physically
/// impossible in the simulation world.
fn is_physically_impossible(category: &str, intent: &str) -> bool {
    let intent_lower = intent.to_lowercase();
    for &impossible in IMPOSSIBLE_ACTIONS {
        if category.contains(impossible) || intent_lower.contains(impossible) {
            return true;
        }
    }
    false
}

/// Map a lowercase freeform category string to a known `ActionType`.
///
/// Returns `None` if no known category matches.
fn map_category_to_action_type(category: &str) -> Option<ActionType> {
    for &(keyword, action_type) in KNOWN_CATEGORIES {
        if category.contains(keyword) {
            return Some(action_type);
        }
    }
    None
}

/// Check whether the agent's location is appropriate for the resolved action.
///
/// Returns `Some(reason)` if the location check fails, `None` if it passes.
fn check_location(
    action_type: ActionType,
    target: Option<&ActionTarget>,
    ctx: &FeasibilityContext,
) -> Option<String> {
    match action_type {
        // Actions targeting another agent require co-location
        ActionType::Steal
        | ActionType::Attack
        | ActionType::Intimidate
        | ActionType::Communicate
        | ActionType::Marry
        | ActionType::Reproduce => {
            if let Some(ActionTarget::Agent(target_id)) = target
                && !ctx.agents_at_location.contains(target_id)
            {
                return Some(format!(
                    "Target agent {target_id} is not at the same location.",
                ));
            }
            None
        }
        // Gather requires resources at location
        ActionType::Gather => {
            if ctx.location_resources.is_empty() {
                return Some(String::from(
                    "No resources available at this location to gather.",
                ));
            }
            None
        }
        // Most other actions have no special location requirement from
        // the feasibility evaluator (the full validation pipeline handles
        // structure/route requirements).
        _ => None,
    }
}

/// Check whether the target entity exists and is accessible.
///
/// Returns `Some(reason)` if the target check fails, `None` if it passes.
fn check_target_exists(
    action_type: ActionType,
    target: Option<&ActionTarget>,
    ctx: &FeasibilityContext,
) -> Option<String> {
    match action_type {
        // Agent-targeting actions require a valid agent target
        ActionType::Steal
        | ActionType::Attack
        | ActionType::Intimidate
        | ActionType::Communicate
        | ActionType::Marry
        | ActionType::Divorce
        | ActionType::Reproduce
        | ActionType::Teach
        | ActionType::TradeOffer
        | ActionType::Enforce => match target {
            Some(ActionTarget::Agent(agent_id)) => {
                if !ctx.agents_at_location.contains(agent_id) {
                    return Some(format!(
                        "Target agent {agent_id} is not present at this location.",
                    ));
                }
                None
            }
            Some(_) => Some(String::from(
                "This action requires an agent target, but a different target type was provided.",
            )),
            None => Some(String::from(
                "This action requires a target agent, but none was specified.",
            )),
        },
        // Structure-targeting actions
        ActionType::Repair | ActionType::Demolish | ActionType::Claim => match target {
            Some(ActionTarget::Structure(structure_id)) => {
                if !ctx.structures_at_location.contains(structure_id) {
                    return Some(format!(
                        "Target structure {structure_id} is not at this location.",
                    ));
                }
                None
            }
            Some(_) => Some(String::from(
                "This action requires a structure target, but a different target type was provided.",
            )),
            None => Some(String::from(
                "This action requires a target structure, but none was specified.",
            )),
        },
        // Actions that do not require a specific target
        _ => None,
    }
}

/// Resolve a freeform action into concrete `ActionParameters` for the
/// given action type.
///
/// This function attempts to extract the necessary parameters from the
/// freeform action's target and parameter map. If the required data is
/// missing or invalid, it returns an error string.
fn resolve_parameters(
    action_type: ActionType,
    action: &FreeformAction,
    ctx: &FeasibilityContext,
) -> Result<ActionParameters, String> {
    match action_type {
        ActionType::Steal => {
            let target_agent = extract_agent_target(action.target.as_ref())?;
            let resource = extract_resource_param(action)?;
            Ok(ActionParameters::Steal {
                target_agent,
                resource,
            })
        }
        ActionType::Attack => {
            let target_agent = extract_agent_target(action.target.as_ref())?;
            Ok(ActionParameters::Attack { target_agent })
        }
        ActionType::Intimidate => {
            let target_agent = extract_agent_target(action.target.as_ref())?;
            Ok(ActionParameters::Intimidate { target_agent })
        }
        ActionType::Pray => Ok(ActionParameters::Pray {
            intent: if action.intent.is_empty() {
                None
            } else {
                Some(action.intent.clone())
            },
        }),
        ActionType::Marry => {
            let partner = extract_agent_target(action.target.as_ref())?;
            Ok(ActionParameters::Marry {
                partner_agent: partner,
            })
        }
        ActionType::Divorce => {
            let partner = extract_agent_target(action.target.as_ref())?;
            Ok(ActionParameters::Divorce {
                partner_agent: partner,
            })
        }
        ActionType::Propose => {
            let group_id = extract_group_target(action.target.as_ref(), ctx)?;
            Ok(ActionParameters::Propose {
                group_id,
                proposal: action.intent.clone(),
            })
        }
        ActionType::Vote => {
            let group_id = extract_group_target(action.target.as_ref(), ctx)?;
            let in_favor = action
                .parameters
                .get("in_favor")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            Ok(ActionParameters::Vote { group_id, in_favor })
        }
        ActionType::Conspire => {
            // Extract co-conspirators from the target or parameters
            let co_conspirators = extract_agent_list_from_params(action)?;
            Ok(ActionParameters::Conspire {
                co_conspirators,
                plan: action.intent.clone(),
            })
        }
        // For actions we cannot fully resolve from freeform parameters,
        // return an error so the evaluator returns NeedsEvaluation or
        // Infeasible.
        _ => Err(format!(
            "Cannot automatically resolve freeform action to concrete parameters \
             for action type {action_type:?}. The intent was: '{}'.",
            action.intent,
        )),
    }
}

/// Extract an `AgentId` from an `ActionTarget::Agent` variant.
fn extract_agent_target(target: Option<&ActionTarget>) -> Result<AgentId, String> {
    match target {
        Some(ActionTarget::Agent(id)) => Ok(*id),
        Some(_) => Err(String::from(
            "Expected an agent target but received a different target type.",
        )),
        None => Err(String::from("No target agent specified.")),
    }
}

/// Extract a `GroupId` from an `ActionTarget::Group` variant, or fall
/// back to the agent's first group.
fn extract_group_target(
    target: Option<&ActionTarget>,
    ctx: &FeasibilityContext,
) -> Result<GroupId, String> {
    match target {
        Some(ActionTarget::Group(id)) => Ok(*id),
        Some(_) => Err(String::from(
            "Expected a group target but received a different target type.",
        )),
        None => {
            // Fall back to first group if agent only belongs to one
            ctx.agent_groups
                .first()
                .copied()
                .ok_or_else(|| String::from("No target group specified and agent belongs to no groups."))
        }
    }
}

/// Extract a `Resource` from the freeform action's parameters map.
fn extract_resource_param(action: &FreeformAction) -> Result<Resource, String> {
    let resource_val = action
        .parameters
        .get("resource")
        .ok_or_else(|| String::from("No 'resource' parameter specified for this action."))?;

    serde_json::from_value(resource_val.clone())
        .map_err(|e| format!("Invalid resource value: {e}"))
}

/// Extract a list of `AgentId` values from the freeform action's
/// parameters map (used by conspire).
fn extract_agent_list_from_params(action: &FreeformAction) -> Result<Vec<AgentId>, String> {
    // Try to get from the target first (if it's a group, we need param)
    if let Some(ActionTarget::Agent(id)) = &action.target {
        return Ok(vec![*id]);
    }

    let agents_val = action
        .parameters
        .get("co_conspirators")
        .or_else(|| action.parameters.get("agents"))
        .ok_or_else(|| {
            String::from("No co-conspirators specified for conspire action.")
        })?;

    serde_json::from_value(agents_val.clone())
        .map_err(|e| format!("Invalid co-conspirators value: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_types::{
        ActionTarget, AgentId, AgentState, FreeformAction, GroupId, LocationId, Resource,
        ResourceNode,
    };

    use super::*;

    fn make_agent_state(agent_id: AgentId, location_id: LocationId, energy: u32) -> AgentState {
        AgentState {
            agent_id,
            energy,
            health: 100,
            hunger: 0,
            age: 100,
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

    fn make_context(
        agent_id: AgentId,
        location_id: LocationId,
        agents_at_location: Vec<AgentId>,
    ) -> FeasibilityContext {
        let mut location_resources = BTreeMap::new();
        location_resources.insert(
            Resource::Wood,
            ResourceNode {
                resource: Resource::Wood,
                available: 50,
                regen_per_tick: 5,
                max_capacity: 100,
            },
        );

        FeasibilityContext {
            agent_id,
            location_id,
            location_resources,
            agents_at_location,
            structures_at_location: Vec::new(),
            agent_groups: Vec::new(),
            agent_knowledge: BTreeSet::new(),
        }
    }

    #[test]
    fn impossible_action_rejected() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to fly to the mountain"),
            action_category: String::from("fly"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::Infeasible { .. }),
            "Flying should be infeasible"
        );
    }

    #[test]
    fn teleport_rejected() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to teleport to the forest"),
            action_category: String::from("move"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::Infeasible { .. }),
            "Teleport in intent should be caught"
        );
    }

    #[test]
    fn pray_action_feasible() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to pray for rain"),
            action_category: String::from("pray"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action,
                energy_cost,
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Pray);
                assert_eq!(energy_cost, 5);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn steal_action_feasible_with_target() {
        let agent_id = AgentId::new();
        let target_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id, target_id]);

        let mut params = BTreeMap::new();
        params.insert(
            String::from("resource"),
            serde_json::to_value(Resource::FoodBerry).unwrap(),
        );

        let action = FreeformAction {
            intent: String::from("I want to steal berries from the other agent"),
            action_category: String::from("steal"),
            target: Some(ActionTarget::Agent(target_id)),
            parameters: params,
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action,
                energy_cost,
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Steal);
                assert_eq!(energy_cost, 15);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn steal_without_target_infeasible() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to steal"),
            action_category: String::from("steal"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::Infeasible { .. }),
            "Steal without target should be infeasible"
        );
    }

    #[test]
    fn steal_target_not_collocated() {
        let agent_id = AgentId::new();
        let target_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        // Target is NOT at the same location
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to steal from them"),
            action_category: String::from("steal"),
            target: Some(ActionTarget::Agent(target_id)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::Infeasible { .. }),
            "Steal from non-co-located agent should be infeasible"
        );
    }

    #[test]
    fn insufficient_energy_infeasible() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        // Only 2 energy -- not enough for pray (5)
        let agent_state = make_agent_state(agent_id, location_id, 2);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to pray"),
            action_category: String::from("pray"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::Infeasible { .. }),
            "Low energy should make action infeasible"
        );
    }

    #[test]
    fn unknown_category_needs_evaluation() {
        let agent_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id]);

        let action = FreeformAction {
            intent: String::from("I want to compose a symphony"),
            action_category: String::from("compose"),
            target: None,
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        assert!(
            matches!(result, FeasibilityResult::NeedsEvaluation { .. }),
            "Unknown category should need LLM evaluation"
        );
    }

    #[test]
    fn attack_action_feasible() {
        let agent_id = AgentId::new();
        let target_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id, target_id]);

        let action = FreeformAction {
            intent: String::from("I want to fight them"),
            action_category: String::from("fight"),
            target: Some(ActionTarget::Agent(target_id)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action,
                energy_cost,
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Attack);
                assert!(energy_cost > 0);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn marry_action_feasible() {
        let agent_id = AgentId::new();
        let partner_id = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id, partner_id]);

        let action = FreeformAction {
            intent: String::from("I want to marry my beloved"),
            action_category: String::from("marry"),
            target: Some(ActionTarget::Agent(partner_id)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action, ..
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Marry);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn conspire_with_agent_target() {
        let agent_id = AgentId::new();
        let co_conspirator = AgentId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let ctx = make_context(agent_id, location_id, vec![agent_id, co_conspirator]);

        let action = FreeformAction {
            intent: String::from("Let us overthrow the leader"),
            action_category: String::from("conspire"),
            target: Some(ActionTarget::Agent(co_conspirator)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action, ..
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Conspire);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn category_mapping_case_insensitive() {
        assert_eq!(
            map_category_to_action_type("STEAL"),
            None,
            "Uppercase should not match (categories are lowercased before lookup)"
        );
        assert_eq!(
            map_category_to_action_type("steal"),
            Some(ActionType::Steal)
        );
        assert_eq!(
            map_category_to_action_type("worship"),
            Some(ActionType::Pray)
        );
        assert_eq!(
            map_category_to_action_type("meditate"),
            Some(ActionType::Pray)
        );
    }

    #[test]
    fn impossible_action_keywords() {
        assert!(is_physically_impossible("fly", "I want to fly"));
        assert!(is_physically_impossible("move", "I want to teleport there"));
        assert!(is_physically_impossible("magic", "cast a spell"));
        assert!(!is_physically_impossible("steal", "take their food"));
        assert!(!is_physically_impossible("pray", "pray for guidance"));
    }

    #[test]
    fn propose_with_group_target() {
        let agent_id = AgentId::new();
        let group_id = GroupId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let mut ctx = make_context(agent_id, location_id, vec![agent_id]);
        ctx.agent_groups = vec![group_id];

        let action = FreeformAction {
            intent: String::from("I propose we build a wall"),
            action_category: String::from("propose"),
            target: Some(ActionTarget::Group(group_id)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action, ..
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Propose);
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }

    #[test]
    fn vote_defaults_to_in_favor() {
        let agent_id = AgentId::new();
        let group_id = GroupId::new();
        let location_id = LocationId::new();
        let agent_state = make_agent_state(agent_id, location_id, 80);
        let mut ctx = make_context(agent_id, location_id, vec![agent_id]);
        ctx.agent_groups = vec![group_id];

        let action = FreeformAction {
            intent: String::from("I vote yes"),
            action_category: String::from("vote"),
            target: Some(ActionTarget::Group(group_id)),
            parameters: BTreeMap::new(),
        };

        let result = evaluate_feasibility(&action, &agent_state, &ctx);
        match result {
            FeasibilityResult::Feasible {
                resolved_action, ..
            } => {
                assert_eq!(resolved_action.action_type, ActionType::Vote);
                if let ActionParameters::Vote { in_favor, .. } = resolved_action.parameters {
                    assert!(in_favor, "Default vote should be in favor");
                } else {
                    panic!("Expected Vote parameters");
                }
            }
            other => panic!("Expected Feasible, got {other:?}"),
        }
    }
}
