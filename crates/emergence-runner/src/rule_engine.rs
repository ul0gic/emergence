//! Rule-based decision engine for routine agent actions.
//!
//! Implements a fast-path that bypasses LLM calls for obvious survival
//! decisions. When an agent is starving and has food, the correct action
//! is always "eat." When energy is depleted, the correct action is always
//! "rest." These deterministic decisions do not benefit from LLM inference
//! and can be resolved in microseconds instead of seconds.
//!
//! The night cycle optimization also lives here: sleeping or low-energy
//! agents during `Night` ticks auto-rest without an LLM call.
//!
//! See `build-plan.md` tasks 6.2.1 and 6.2.4.

use emergence_types::{
    ActionParameters, ActionRequest, ActionType, AgentId, Perception, Resource, TimeOfDay,
};
use chrono::Utc;
use tracing::info;

use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Thresholds (kept as constants so operators can find and tune them)
// ---------------------------------------------------------------------------

/// Health threshold below which medicine use is critical.
const CRITICAL_HEALTH: u32 = 20;

/// Thirst threshold for "dying of thirst" -- drink immediately.
const CRITICAL_THIRST: u32 = 80;

/// Hunger threshold for "starving" -- highest priority eating.
const STARVING_HUNGER: u32 = 80;

/// Energy threshold for "exhausted" -- must rest immediately.
const EXHAUSTED_ENERGY: u32 = 10;

/// Thirst threshold for "thirsty" -- should drink when convenient.
const THIRSTY: u32 = 50;

/// Hunger threshold for "very hungry" -- will eat if food available.
const VERY_HUNGRY: u32 = 50;

/// Hunger threshold for proactive food gathering.
const GATHER_FOOD_HUNGER: u32 = 60;

/// Energy threshold for "low energy" -- will rest if no urgent needs.
const LOW_ENERGY: u32 = 25;

/// Energy threshold below which night cycle auto-rest activates.
const NIGHT_REST_ENERGY: u32 = 50;

/// Number of consecutive identical rule firings before escalating to the LLM.
const LOOP_DETECTION_THRESHOLD: u32 = 10;

// ---------------------------------------------------------------------------
// Loop detection state
// ---------------------------------------------------------------------------

/// Tracks per-agent loop detection: (last rule name, consecutive count).
static LOOP_TRACKER: Mutex<Option<HashMap<AgentId, (String, u32)>>> = Mutex::new(None);

/// Record a rule firing for an agent. Returns `true` if the same rule has
/// fired `LOOP_DETECTION_THRESHOLD` or more times consecutively, meaning
/// the caller should skip the rule engine and escalate to the LLM.
fn check_loop_detection(agent_id: AgentId, rule_name: &str) -> bool {
    let Ok(mut guard) = LOOP_TRACKER.lock() else {
        return false; // poisoned mutex -- do not block on it
    };
    let tracker = guard.get_or_insert_with(HashMap::new);

    let entry = tracker.entry(agent_id).or_insert_with(|| (String::new(), 0));
    if entry.0 == rule_name {
        entry.1 = entry.1.saturating_add(1);
    } else {
        entry.0 = String::from(rule_name);
        entry.1 = 1;
    }

    entry.1 >= LOOP_DETECTION_THRESHOLD
}

/// Reset the loop counter for an agent (called when the LLM makes a decision
/// or a different rule fires).
pub fn reset_loop_detection(agent_id: AgentId) {
    let Ok(mut guard) = LOOP_TRACKER.lock() else {
        return;
    };
    if let Some(tracker) = guard.as_mut() {
        tracker.remove(&agent_id);
    }
}

// ---------------------------------------------------------------------------
// Decision source tagging
// ---------------------------------------------------------------------------

/// Indicates where a decision came from, for metrics and the observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionSource {
    /// Decision was made by the LLM backend.
    Llm,
    /// Decision was made by the routine action rule engine.
    RuleEngine,
    /// Decision was made by the night cycle optimization.
    NightCycle,
}

impl DecisionSource {
    /// Human-readable label for logging and metrics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::RuleEngine => "rule_engine",
            Self::NightCycle => "night_cycle",
        }
    }
}

// ---------------------------------------------------------------------------
// Food priority ranking
// ---------------------------------------------------------------------------

/// Food resources ranked from best to worst nutritional value.
/// Cooked > Farmed > Fish > Meat > Berry > Root.
const FOOD_PRIORITY: &[Resource] = &[
    Resource::FoodCooked,
    Resource::FoodFarmed,
    Resource::FoodFish,
    Resource::FoodMeat,
    Resource::FoodBerry,
    Resource::FoodRoot,
];

/// Check whether a resource counts as food.
///
/// Currently used only in tests but kept as a utility for future rule
/// engine extensions (e.g., checking if any inventory item is edible).
#[allow(dead_code)]
const fn is_food(resource: Resource) -> bool {
    matches!(
        resource,
        Resource::FoodCooked
            | Resource::FoodFarmed
            | Resource::FoodFish
            | Resource::FoodMeat
            | Resource::FoodBerry
            | Resource::FoodRoot
    )
}

/// Find the best food in the agent's inventory, returning the resource
/// type and quantity. "Best" is defined by [`FOOD_PRIORITY`] ordering.
fn best_food_in_inventory(
    inventory: &std::collections::BTreeMap<Resource, u32>,
) -> Option<Resource> {
    for &food in FOOD_PRIORITY {
        if let Some(&qty) = inventory.get(&food)
            && qty > 0
        {
            return Some(food);
        }
    }
    None
}

/// Check whether the agent has medicine in inventory.
fn has_medicine(inventory: &std::collections::BTreeMap<Resource, u32>) -> bool {
    inventory
        .get(&Resource::Medicine)
        .copied()
        .unwrap_or(0)
        > 0
}

/// Check whether the agent has water in inventory.
fn has_water(inventory: &std::collections::BTreeMap<Resource, u32>) -> bool {
    inventory
        .get(&Resource::Water)
        .copied()
        .unwrap_or(0)
        > 0
}

/// Check whether the agent is at a location with visible water resources.
fn at_water_source(perception: &Perception) -> bool {
    perception
        .surroundings
        .visible_resources
        .contains_key(&Resource::Water)
}

/// Check whether there is gatherable food at the agent's current location.
///
/// Returns the first food resource visible at the location, preferring
/// better food types according to [`FOOD_PRIORITY`].
fn food_at_location(perception: &Perception) -> Option<Resource> {
    FOOD_PRIORITY
        .iter()
        .find(|&&food| perception.surroundings.visible_resources.contains_key(&food))
        .copied()
}

/// Base gather yield, mirroring the constant from `emergence-agents` costs.
///
/// We duplicate the value here rather than taking a crate dependency to keep
/// the rule engine self-contained. If the costs module value changes, update
/// this constant as well.
const BASE_GATHER_YIELD: u32 = 3;

/// Parse the `carry_load` string (e.g. "26/50") into `(current, max)`.
///
/// Returns `None` if the string is malformed — callers should treat a parse
/// failure as "unknown capacity" and conservatively skip the gather.
fn parse_carry_load(carry_load: &str) -> Option<(u32, u32)> {
    let (current_str, max_str) = carry_load.split_once('/')?;
    let current: u32 = current_str.trim().parse().ok()?;
    let max: u32 = max_str.trim().parse().ok()?;
    Some((current, max))
}

/// Check whether the agent has room in inventory for a gather action.
///
/// Returns `true` if `current_load + BASE_GATHER_YIELD <= max_capacity`.
/// Returns `false` (conservatively) if the carry load cannot be parsed.
fn has_inventory_room(perception: &Perception) -> bool {
    let Some((current, max)) = parse_carry_load(&perception.self_state.carry_load) else {
        return false;
    };
    current.checked_add(BASE_GATHER_YIELD).is_some_and(|total| total <= max)
}

/// Check whether the given action name is in the agent's available actions list.
fn action_available(perception: &Perception, action_name: &str) -> bool {
    let lower = action_name.to_lowercase();
    perception
        .available_actions
        .iter()
        .any(|a| a.to_lowercase() == lower)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Attempts to determine an action without calling the LLM.
///
/// Returns `Some((rule_name, ActionRequest))` if an obvious action exists,
/// `None` if the LLM is needed for a non-trivial decision. Rules are
/// evaluated in strict priority order (highest urgency first).
///
/// Loop detection: if the same rule fires `LOOP_DETECTION_THRESHOLD` times
/// in a row for the same agent, returns `None` to escalate to the LLM.
///
/// # Rules (in priority order)
///
/// 1. **Critical health**: health < 20 and has medicine -- use medicine (eat)
/// 2. **Dying of thirst**: thirst >= 80 and water available -- drink
/// 3. **Starving**: hunger >= 80 and has food -- eat best food
/// 4. **Exhausted**: energy <= 10 -- rest
/// 5. **Thirsty**: thirst >= 50 and water available -- drink
/// 6. **Very hungry**: hunger >= 50 and has food -- eat
/// 7. **Very hungry, no food, food at location**: hunger >= 60 -- gather food
/// 8. **Low energy**: energy <= 25 and no urgent needs -- rest
pub fn try_routine_action(perception: &Perception) -> Option<ActionRequest> {
    let agent_id = perception.self_state.id;

    // Try to find a matching rule, then apply loop detection.
    let candidate = try_routine_action_inner(perception);

    if let Some((rule_name, action)) = candidate {
        // Loop detection: if this rule has fired too many times, escalate.
        if check_loop_detection(agent_id, &rule_name) {
            info!(
                agent_id = %agent_id,
                rule = rule_name,
                "rule engine: loop detected ({LOOP_DETECTION_THRESHOLD}+ consecutive), escalating to LLM"
            );
            return None;
        }
        return Some(action);
    }

    // No routine action applies -- reset loop counter and let LLM decide.
    reset_loop_detection(agent_id);
    None
}

/// Inner implementation of routine action matching. Returns the rule name
/// and action request if a rule matches, or `None` if no rule applies.
fn try_routine_action_inner(perception: &Perception) -> Option<(String, ActionRequest)> {
    let state = &perception.self_state;
    let inventory = &state.inventory;
    let agent_id = state.id;
    let tick = perception.tick;

    // Rule 1: Critical health -- use medicine
    if state.health < CRITICAL_HEALTH
        && has_medicine(inventory)
        && action_available(perception, "eat")
    {
        info!(
            agent_id = %agent_id,
            health = state.health,
            rule = "critical_health",
            "rule engine: using medicine (health critical)"
        );
        return Some(("critical_health".to_owned(), make_eat_action(agent_id, tick, Resource::Medicine)));
    }

    // Rule 2: Dying of thirst -- drink immediately
    if state.thirst >= CRITICAL_THIRST
        && (at_water_source(perception) || has_water(inventory))
        && action_available(perception, "drink")
    {
        info!(
            agent_id = %agent_id,
            thirst = state.thirst,
            rule = "critical_thirst",
            "rule engine: drinking (dying of thirst)"
        );
        return Some(("critical_thirst".to_owned(), make_drink_action(agent_id, tick)));
    }

    // Rule 3: Starving -- eat best food
    if state.hunger >= STARVING_HUNGER
        && action_available(perception, "eat")
        && let Some(food) = best_food_in_inventory(inventory)
    {
        info!(
            agent_id = %agent_id,
            hunger = state.hunger,
            food = ?food,
            rule = "starving",
            "rule engine: eating (starving)"
        );
        return Some(("starving".to_owned(), make_eat_action(agent_id, tick, food)));
    }

    // Rule 4: Exhausted -- rest
    if state.energy <= EXHAUSTED_ENERGY && action_available(perception, "rest") {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            rule = "exhausted",
            "rule engine: resting (exhausted)"
        );
        return Some(("exhausted".to_owned(), make_rest_action(agent_id, tick)));
    }

    // Rule 5: Thirsty -- drink when convenient
    if state.thirst >= THIRSTY
        && (at_water_source(perception) || has_water(inventory))
        && action_available(perception, "drink")
    {
        info!(
            agent_id = %agent_id,
            thirst = state.thirst,
            rule = "thirsty",
            "rule engine: drinking (thirsty)"
        );
        return Some(("thirsty".to_owned(), make_drink_action(agent_id, tick)));
    }

    // Rule 6: Very hungry -- eat if food available
    if state.hunger >= VERY_HUNGRY
        && action_available(perception, "eat")
        && let Some(food) = best_food_in_inventory(inventory)
    {
        info!(
            agent_id = %agent_id,
            hunger = state.hunger,
            food = ?food,
            rule = "very_hungry",
            "rule engine: eating (very hungry)"
        );
        return Some(("very_hungry".to_owned(), make_eat_action(agent_id, tick, food)));
    }

    // Rule 7: Very hungry but no food -- gather food if available at location
    // Guard: skip gather if inventory is full (would be rejected by validation).
    if state.hunger >= GATHER_FOOD_HUNGER
        && best_food_in_inventory(inventory).is_none()
        && has_inventory_room(perception)
        && action_available(perception, "gather")
        && let Some(food) = food_at_location(perception)
    {
        info!(
            agent_id = %agent_id,
            hunger = state.hunger,
            food = ?food,
            rule = "gather_food",
            "rule engine: gathering food (hungry, no food in inventory)"
        );
        return Some(("gather_food".to_owned(), make_gather_action(agent_id, tick, food)));
    }

    // Rule 8: Low energy -- rest if no urgent survival needs
    if state.energy <= LOW_ENERGY && action_available(perception, "rest") {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            rule = "low_energy",
            "rule engine: resting (low energy)"
        );
        return Some(("low_energy".to_owned(), make_rest_action(agent_id, tick)));
    }

    // No routine action applies -- LLM is needed.
    None
}

/// Check whether an agent should auto-rest due to the night cycle.
///
/// Returns `Some(ActionRequest)` with a rest action if:
/// - It is `Night` and the agent's energy is below the night rest threshold, OR
/// - The agent appears to already be resting/sleeping (no urgent needs at night)
///
/// Returns `None` if the agent should still get an LLM decision at night
/// (e.g., high energy, things happening around them).
pub fn try_night_cycle_rest(perception: &Perception) -> Option<ActionRequest> {
    if perception.time_of_day != TimeOfDay::Night {
        return None;
    }

    let state = &perception.self_state;
    let agent_id = state.id;
    let tick = perception.tick;

    // Night + low energy = auto-rest
    if state.energy < NIGHT_REST_ENERGY && action_available(perception, "rest") {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            time_of_day = ?perception.time_of_day,
            rule = "night_rest",
            "night cycle: auto-resting (night + low energy)"
        );
        return Some(make_rest_action(agent_id, tick));
    }

    // Night + no urgent needs + nothing interesting happening = auto-rest
    // "Nothing interesting" = no other agents, no messages, no notifications
    let nothing_happening = perception.surroundings.agents_here.is_empty()
        && perception.surroundings.messages_here.is_empty()
        && perception.notifications.is_empty();

    if nothing_happening
        && state.hunger < VERY_HUNGRY
        && action_available(perception, "rest")
    {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            time_of_day = ?perception.time_of_day,
            rule = "night_quiet",
            "night cycle: auto-resting (quiet night, no urgent needs)"
        );
        return Some(make_rest_action(agent_id, tick));
    }

    None
}

// ---------------------------------------------------------------------------
// Action constructors
// ---------------------------------------------------------------------------

/// Build an `Eat` action request.
fn make_eat_action(agent_id: AgentId, tick: u64, food: Resource) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::Eat,
        parameters: ActionParameters::Eat { food_type: food },
        submitted_at: Utc::now(),
        goal_updates: Vec::new(),
    }
}

/// Build a `Drink` action request.
fn make_drink_action(agent_id: AgentId, tick: u64) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::Drink,
        parameters: ActionParameters::Drink,
        submitted_at: Utc::now(),
        goal_updates: Vec::new(),
    }
}

/// Build a `Gather` action request.
fn make_gather_action(agent_id: AgentId, tick: u64, resource: Resource) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::Gather,
        parameters: ActionParameters::Gather { resource },
        submitted_at: Utc::now(),
        goal_updates: Vec::new(),
    }
}

/// Build a `Rest` action request.
fn make_rest_action(agent_id: AgentId, tick: u64) -> ActionRequest {
    ActionRequest {
        agent_id,
        tick,
        action_type: ActionType::Rest,
        parameters: ActionParameters::Rest,
        submitted_at: Utc::now(),
        goal_updates: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use emergence_types::{
        Season, SelfState, Sex, Surroundings, Weather,
    };

    /// Build a minimal perception with customizable vitals and inventory.
    /// `thirst` defaults to 0 for backward-compatible tests.
    fn make_perception(
        energy: u32,
        health: u32,
        hunger: u32,
        inventory: BTreeMap<Resource, u32>,
        time_of_day: TimeOfDay,
        visible_resources: BTreeMap<Resource, String>,
        available_actions: Vec<String>,
    ) -> Perception {
        make_perception_with_thirst(
            energy, health, hunger, 0,
            inventory, time_of_day, visible_resources, available_actions,
        )
    }

    /// Build a perception with explicit thirst control.
    fn make_perception_with_thirst(
        energy: u32,
        health: u32,
        hunger: u32,
        thirst: u32,
        inventory: BTreeMap<Resource, u32>,
        time_of_day: TimeOfDay,
        visible_resources: BTreeMap<Resource, String>,
        available_actions: Vec<String>,
    ) -> Perception {
        Perception {
            tick: 100,
            time_of_day,
            season: Season::Summer,
            weather: Weather::Clear,
            self_state: SelfState {
                id: AgentId::new(),
                name: "TestAgent".to_owned(),
                sex: Sex::Male,
                age: 10,
                energy,
                health,
                hunger,
                thirst,
                location_name: "Forest".to_owned(),
                inventory,
                carry_load: "5/50".to_owned(),
                active_goals: vec!["survive".to_owned()],
                known_skills: Vec::new(),
            },
            surroundings: Surroundings {
                location_description: "A test location".to_owned(),
                visible_resources,
                structures_here: Vec::new(),
                agents_here: Vec::new(),
                messages_here: Vec::new(),
            },
            known_routes: Vec::new(),
            recent_memory: Vec::new(),
            available_actions,
            notifications: Vec::new(),
            personality: None,
        }
    }

    fn default_actions() -> Vec<String> {
        vec![
            "gather".to_owned(),
            "eat".to_owned(),
            "drink".to_owned(),
            "rest".to_owned(),
            "move".to_owned(),
        ]
    }

    // -----------------------------------------------------------------------
    // Rule 1: Critical health
    // -----------------------------------------------------------------------

    #[test]
    fn critical_health_uses_medicine() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Medicine, 2);
        let p = make_perception(
            50, 15, 30, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::Medicine }
        ));
    }

    #[test]
    fn critical_health_no_medicine_falls_through() {
        let p = make_perception(
            50, 15, 30, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Health is critical but no medicine -- should not trigger rule 1
        // but no other rule triggers either (hunger 30, energy 50)
        let result = try_routine_action(&p);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Rule 2: Dying of thirst (critical thirst)
    // -----------------------------------------------------------------------

    #[test]
    fn critical_thirst_drinks_at_water_source() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::Water, "plentiful".to_owned());
        let p = make_perception_with_thirst(
            50, 80, 30, 85,
            BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    #[test]
    fn critical_thirst_drinks_from_inventory() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Water, 3);
        let p = make_perception_with_thirst(
            50, 80, 30, 85,
            inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    #[test]
    fn critical_thirst_no_water_does_not_drink() {
        let p = make_perception_with_thirst(
            50, 80, 30, 85,
            BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Critical thirst but no water source and no water in inventory
        let result = try_routine_action(&p);
        // Falls through: not starving (hunger 30), not exhausted (energy 50),
        // not thirsty with water, not very hungry, not gather food (hunger < 60)
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Rule 3: Starving
    // -----------------------------------------------------------------------

    #[test]
    fn starving_eats_best_food() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 5);
        inv.insert(Resource::FoodCooked, 2);
        let p = make_perception(
            50, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
        // Should pick cooked food (best) over berries
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::FoodCooked }
        ));
    }

    #[test]
    fn starving_no_food_does_not_eat() {
        let p = make_perception(
            50, 80, 85, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Starving but no food -- falls through to lower rules
        let result = try_routine_action(&p);
        // No food, thirst 0 (no thirst rules), not exhausted (energy 50 > 10),
        // not very hungry with food, gather food might trigger (hunger 85 >= 60,
        // no food in inventory, but no food at location either) -> low energy (50 > 25)
        // Returns None
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Rule 5: Thirsty (moderate)
    // -----------------------------------------------------------------------

    #[test]
    fn thirsty_drinks_at_water_source() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::Water, "plentiful".to_owned());
        let p = make_perception_with_thirst(
            50, 80, 30, 55,
            BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    #[test]
    fn thirsty_no_water_does_not_drink() {
        let p = make_perception_with_thirst(
            50, 80, 30, 55,
            BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Thirsty but no water
        let result = try_routine_action(&p);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Rule 4: Exhausted
    // -----------------------------------------------------------------------

    #[test]
    fn exhausted_rests() {
        let p = make_perception(
            5, 80, 20, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn exhausted_boundary_at_10() {
        let p = make_perception(
            10, 80, 20, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn not_exhausted_at_11() {
        let p = make_perception(
            11, 80, 20, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Energy 11 > 10 so not exhausted, hunger 20 < 50 so not hungry, energy 11 < 25 triggers low energy
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest); // low energy rule
    }

    // -----------------------------------------------------------------------
    // Rule 5: Very hungry
    // -----------------------------------------------------------------------

    #[test]
    fn very_hungry_eats() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodMeat, 3);
        let p = make_perception(
            50, 80, 55, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::FoodMeat }
        ));
    }

    // -----------------------------------------------------------------------
    // Rule 6: Low energy
    // -----------------------------------------------------------------------

    #[test]
    fn low_energy_rests() {
        let p = make_perception(
            20, 80, 10, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn low_energy_boundary_at_25() {
        let p = make_perception(
            25, 80, 10, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn adequate_energy_no_bypass() {
        let p = make_perception(
            60, 80, 10, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_none(), "healthy agent should not trigger any rule");
    }

    // -----------------------------------------------------------------------
    // Priority ordering: critical health > critical thirst > starving > exhausted
    // -----------------------------------------------------------------------

    #[test]
    fn critical_health_takes_priority_over_starving() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Medicine, 1);
        inv.insert(Resource::FoodCooked, 5);
        // Health critical AND starving -- medicine should win
        let p = make_perception(
            5, 10, 90, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::Medicine }
        ));
    }

    #[test]
    fn starving_takes_priority_over_exhausted() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 3);
        // Starving AND exhausted -- eating should win over resting
        let p = make_perception(
            5, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
    }

    // -----------------------------------------------------------------------
    // Action availability checks
    // -----------------------------------------------------------------------

    #[test]
    fn eat_not_available_skips_eating() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 5);
        // Starving with food but "eat" not in available actions
        let actions = vec!["gather".to_owned(), "rest".to_owned(), "move".to_owned()];
        let p = make_perception(
            50, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), actions,
        );
        let result = try_routine_action(&p);
        // Can't eat, falls through. Not dehydrated enough for drink, not exhausted.
        // No low energy either (50 > 25).
        assert!(result.is_none());
    }

    #[test]
    fn rest_not_available_skips_resting() {
        // Exhausted but rest not in available actions
        let actions = vec!["gather".to_owned(), "eat".to_owned(), "move".to_owned()];
        let p = make_perception(
            5, 80, 20, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), actions,
        );
        let result = try_routine_action(&p);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Food priority ordering
    // -----------------------------------------------------------------------

    #[test]
    fn food_priority_prefers_cooked_over_raw() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodRoot, 10);
        inv.insert(Resource::FoodCooked, 1);
        inv.insert(Resource::FoodBerry, 5);
        let p = make_perception(
            50, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::FoodCooked }
        ));
    }

    #[test]
    fn food_priority_farmed_over_fish() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodFish, 5);
        inv.insert(Resource::FoodFarmed, 2);
        let p = make_perception(
            50, 80, 55, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::FoodFarmed }
        ));
    }

    #[test]
    fn zero_quantity_food_ignored() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodCooked, 0);
        inv.insert(Resource::FoodBerry, 3);
        let p = make_perception(
            50, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        // Cooked has qty 0, should pick berry
        assert!(matches!(
            action.parameters,
            ActionParameters::Eat { food_type: Resource::FoodBerry }
        ));
    }

    // -----------------------------------------------------------------------
    // Night cycle optimization
    // -----------------------------------------------------------------------

    #[test]
    fn night_low_energy_auto_rests() {
        let p = make_perception(
            40, 80, 20, BTreeMap::new(), TimeOfDay::Night,
            BTreeMap::new(), default_actions(),
        );
        let result = try_night_cycle_rest(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn night_high_energy_quiet_auto_rests() {
        // Night, high energy, but nothing happening and not hungry
        let p = make_perception(
            80, 80, 20, BTreeMap::new(), TimeOfDay::Night,
            BTreeMap::new(), default_actions(),
        );
        let result = try_night_cycle_rest(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_drink_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Rest);
    }

    #[test]
    fn night_with_agents_nearby_needs_llm() {
        use emergence_types::VisibleAgent;
        let mut p = make_perception(
            80, 80, 20, BTreeMap::new(), TimeOfDay::Night,
            BTreeMap::new(), default_actions(),
        );
        p.surroundings.agents_here.push(VisibleAgent {
            id: emergence_types::AgentId::new(),
            name: "Stranger".to_owned(),
            sex: Sex::Male,
            relationship: "unknown".to_owned(),
            activity: "watching".to_owned(),
        });
        let result = try_night_cycle_rest(&p);
        // Other agents nearby at night -- should NOT auto-rest, needs LLM
        // Energy 80 >= 50 so night_low_energy doesn't trigger
        assert!(result.is_none());
    }

    #[test]
    fn night_hungry_needs_llm() {
        // Night but very hungry -- might want to eat, not rest
        let p = make_perception(
            80, 80, 55, BTreeMap::new(), TimeOfDay::Night,
            BTreeMap::new(), default_actions(),
        );
        let result = try_night_cycle_rest(&p);
        // Energy 80 >= 50 so not low energy at night
        // hunger >= 50 so "nothing_happening" path is blocked
        assert!(result.is_none());
    }

    #[test]
    fn daytime_no_night_cycle() {
        let p = make_perception(
            30, 80, 20, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_night_cycle_rest(&p);
        assert!(result.is_none(), "night cycle should not trigger during daytime");
    }

    #[test]
    fn night_with_notifications_needs_llm() {
        let mut p = make_perception(
            80, 80, 20, BTreeMap::new(), TimeOfDay::Night,
            BTreeMap::new(), default_actions(),
        );
        p.notifications.push("Winter is approaching!".to_owned());
        let result = try_night_cycle_rest(&p);
        // Notifications present -- not a quiet night
        // Energy 80 >= 50 so night_low_energy doesn't trigger
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn is_food_recognizes_all_food_types() {
        assert!(is_food(Resource::FoodCooked));
        assert!(is_food(Resource::FoodFarmed));
        assert!(is_food(Resource::FoodFish));
        assert!(is_food(Resource::FoodMeat));
        assert!(is_food(Resource::FoodBerry));
        assert!(is_food(Resource::FoodRoot));
        assert!(!is_food(Resource::Wood));
        assert!(!is_food(Resource::Water));
        assert!(!is_food(Resource::Medicine));
    }

    #[test]
    fn best_food_empty_inventory() {
        let inv = BTreeMap::new();
        assert!(best_food_in_inventory(&inv).is_none());
    }

    #[test]
    fn best_food_single_item() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodRoot, 5);
        assert_eq!(best_food_in_inventory(&inv), Some(Resource::FoodRoot));
    }

    #[test]
    fn action_tick_and_agent_id_correct() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 1);
        let p = make_perception(
            50, 80, 85, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let agent_id = p.self_state.id;
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.agent_id, agent_id);
        assert_eq!(action.tick, 100);
    }

    #[test]
    fn decision_source_labels() {
        assert_eq!(DecisionSource::Llm.as_str(), "llm");
        assert_eq!(DecisionSource::RuleEngine.as_str(), "rule_engine");
        assert_eq!(DecisionSource::NightCycle.as_str(), "night_cycle");
    }

    // -----------------------------------------------------------------------
    // Rule 7: Gather food when hungry with no food in inventory
    // -----------------------------------------------------------------------

    #[test]
    fn gather_food_when_hungry_no_inventory() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Gather);
        assert!(matches!(
            action.parameters,
            ActionParameters::Gather { resource: Resource::FoodBerry }
        ));
    }

    #[test]
    fn no_gather_when_food_in_inventory() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 3);
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        // Hunger 65, has food -> should eat (very_hungry rule), not gather
        let p = make_perception(
            50, 80, 65, inv, TimeOfDay::Morning,
            vis, default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
    }

    #[test]
    fn no_gather_when_no_food_at_location() {
        let p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Hungry, no food in inventory, no food at location -> nothing to do
        let result = try_routine_action(&p);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Inventory capacity checks
    // -----------------------------------------------------------------------

    #[test]
    fn parse_carry_load_valid() {
        assert_eq!(parse_carry_load("26/50"), Some((26, 50)));
        assert_eq!(parse_carry_load("0/50"), Some((0, 50)));
        assert_eq!(parse_carry_load("50/50"), Some((50, 50)));
        assert_eq!(parse_carry_load("0/0"), Some((0, 0)));
    }

    #[test]
    fn parse_carry_load_with_whitespace() {
        assert_eq!(parse_carry_load(" 10 / 50 "), Some((10, 50)));
    }

    #[test]
    fn parse_carry_load_malformed() {
        assert_eq!(parse_carry_load(""), None);
        assert_eq!(parse_carry_load("abc"), None);
        assert_eq!(parse_carry_load("10/"), None);
        assert_eq!(parse_carry_load("/50"), None);
        assert_eq!(parse_carry_load("ten/fifty"), None);
    }

    #[test]
    fn no_gather_when_inventory_full() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let mut p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        // Set carry load to exactly full
        p.self_state.carry_load = "50/50".to_owned();
        let result = try_routine_action(&p);
        // Inventory full -> gather skipped -> no other rule fires -> None
        assert!(result.is_none());
    }

    #[test]
    fn no_gather_when_inventory_nearly_full() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let mut p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        // Only 2 units of room but BASE_GATHER_YIELD is 3 -> no room
        p.self_state.carry_load = "48/50".to_owned();
        let result = try_routine_action(&p);
        assert!(result.is_none());
    }

    #[test]
    fn gather_when_inventory_has_exact_room() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let mut p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        // Exactly BASE_GATHER_YIELD (3) units of room -> gather should fire
        p.self_state.carry_load = "47/50".to_owned();
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Gather);
    }

    #[test]
    fn hungry_with_food_eats_even_when_inventory_full() {
        // Agent is hungry AND has food in inventory AND inventory is full.
        // Rule 6 (eat) should fire before Rule 7 (gather) is even considered.
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 5);
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let mut p = make_perception(
            50, 80, 65, inv, TimeOfDay::Morning,
            vis, default_actions(),
        );
        p.self_state.carry_load = "50/50".to_owned();
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        // Eats (Rule 6) rather than attempting to gather
        assert_eq!(action.action_type, ActionType::Eat);
    }

    #[test]
    fn malformed_carry_load_skips_gather() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::FoodBerry, "plentiful".to_owned());
        let mut p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        p.self_state.carry_load = "broken".to_owned();
        let result = try_routine_action(&p);
        // Malformed carry_load -> has_inventory_room returns false -> gather skipped
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Thirst-hunger independence: drinking does NOT affect hunger rule
    // -----------------------------------------------------------------------

    #[test]
    fn hungry_agent_eats_not_drinks() {
        // High hunger, low thirst: should eat, not drink
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 3);
        inv.insert(Resource::Water, 5);
        let p = make_perception_with_thirst(
            50, 80, 85, 10,
            inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Eat);
    }

    #[test]
    fn thirsty_agent_drinks_not_eats() {
        // Low hunger, high thirst: should drink, not eat
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 3);
        inv.insert(Resource::Water, 5);
        let p = make_perception_with_thirst(
            50, 80, 10, 85,
            inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    // -----------------------------------------------------------------------
    // Loop detection
    // -----------------------------------------------------------------------

    #[test]
    fn loop_detection_escalates_after_threshold() {
        let agent_id = AgentId::new();
        // Reset any prior state
        reset_loop_detection(agent_id);

        // Fire the same rule LOOP_DETECTION_THRESHOLD - 1 times: should not trigger
        for _ in 0..LOOP_DETECTION_THRESHOLD.saturating_sub(1) {
            assert!(!check_loop_detection(agent_id, "test_rule"));
        }
        // The Nth firing should trigger
        assert!(check_loop_detection(agent_id, "test_rule"));

        // Reset and verify it clears
        reset_loop_detection(agent_id);
        assert!(!check_loop_detection(agent_id, "test_rule"));
    }

    #[test]
    fn loop_detection_resets_on_different_rule() {
        let agent_id = AgentId::new();
        reset_loop_detection(agent_id);

        for _ in 0..5 {
            assert!(!check_loop_detection(agent_id, "rule_a"));
        }
        // Switch to a different rule -- counter resets
        assert!(!check_loop_detection(agent_id, "rule_b"));
        // Clean up
        reset_loop_detection(agent_id);
    }
}
