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

// ---------------------------------------------------------------------------
// Thresholds (kept as constants so operators can find and tune them)
// ---------------------------------------------------------------------------

/// Health threshold below which medicine use is critical.
const CRITICAL_HEALTH: u32 = 20;

/// Hunger threshold for "starving" -- highest priority eating.
const STARVING_HUNGER: u32 = 80;

/// Hunger threshold for "dehydrated" -- triggers drinking.
const DEHYDRATED_HUNGER: u32 = 60;

/// Energy threshold for "exhausted" -- must rest immediately.
const EXHAUSTED_ENERGY: u32 = 10;

/// Hunger threshold for "very hungry" -- will eat if food available.
const VERY_HUNGRY: u32 = 50;

/// Energy threshold for "low energy" -- will rest if no urgent needs.
const LOW_ENERGY: u32 = 25;

/// Energy threshold below which night cycle auto-rest activates.
const NIGHT_REST_ENERGY: u32 = 50;

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
/// Returns `Some(ActionRequest)` if an obvious action exists, `None` if the
/// LLM is needed for a non-trivial decision. Rules are evaluated in strict
/// priority order (highest urgency first).
///
/// # Rules (in priority order)
///
/// 1. **Critical health**: health < 20 and has medicine -- use medicine (eat)
/// 2. **Starving**: hunger >= 80 and has food -- eat best food
/// 3. **Dehydrated**: hunger >= 60 and at water source or has water -- drink
/// 4. **Exhausted**: energy <= 10 -- rest
/// 5. **Very hungry**: hunger >= 50 and has food -- eat
/// 6. **Low energy**: energy <= 25 and no urgent needs -- rest
pub fn try_routine_action(perception: &Perception) -> Option<ActionRequest> {
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
        return Some(make_eat_action(agent_id, tick, Resource::Medicine));
    }

    // Rule 2: Starving -- eat best food
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
        return Some(make_eat_action(agent_id, tick, food));
    }

    // Rule 3: Dehydrated -- drink
    if state.hunger >= DEHYDRATED_HUNGER
        && (at_water_source(perception) || has_water(inventory))
        && action_available(perception, "drink")
    {
        info!(
            agent_id = %agent_id,
            hunger = state.hunger,
            rule = "dehydrated",
            "rule engine: drinking (dehydrated)"
        );
        return Some(make_drink_action(agent_id, tick));
    }

    // Rule 4: Exhausted -- rest
    if state.energy <= EXHAUSTED_ENERGY && action_available(perception, "rest") {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            rule = "exhausted",
            "rule engine: resting (exhausted)"
        );
        return Some(make_rest_action(agent_id, tick));
    }

    // Rule 5: Very hungry -- eat if food available
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
        return Some(make_eat_action(agent_id, tick, food));
    }

    // Rule 6: Low energy -- rest if no urgent survival needs
    if state.energy <= LOW_ENERGY && action_available(perception, "rest") {
        info!(
            agent_id = %agent_id,
            energy = state.energy,
            rule = "low_energy",
            "rule engine: resting (low energy)"
        );
        return Some(make_rest_action(agent_id, tick));
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
        Season, SelfState, Surroundings, Weather,
    };

    /// Build a minimal perception with customizable vitals and inventory.
    fn make_perception(
        energy: u32,
        health: u32,
        hunger: u32,
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
                age: 10,
                energy,
                health,
                hunger,
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
    // Rule 2: Starving
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
        // No food, but hunger >= 60 and no water -> falls to exhausted check (energy 50 > 10)
        // Then very hungry check (hunger 85 >= 50 but no food)
        // Then low energy check (energy 50 > 25)
        // Returns None
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // Rule 3: Dehydrated
    // -----------------------------------------------------------------------

    #[test]
    fn dehydrated_drinks_at_water_source() {
        let mut vis = BTreeMap::new();
        vis.insert(Resource::Water, "plentiful".to_owned());
        let p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            vis, default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    #[test]
    fn dehydrated_drinks_from_inventory() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Water, 3);
        let p = make_perception(
            50, 80, 65, inv, TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        let result = try_routine_action(&p);
        assert!(result.is_some());
        let action = result.unwrap_or_else(|| make_rest_action(AgentId::new(), 0));
        assert_eq!(action.action_type, ActionType::Drink);
    }

    #[test]
    fn dehydrated_no_water_does_not_drink() {
        let p = make_perception(
            50, 80, 65, BTreeMap::new(), TimeOfDay::Morning,
            BTreeMap::new(), default_actions(),
        );
        // Dehydrated but no water source and no water in inventory
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
    // Priority ordering: critical health > starving > dehydrated > exhausted
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
            name: "Stranger".to_owned(),
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
}
