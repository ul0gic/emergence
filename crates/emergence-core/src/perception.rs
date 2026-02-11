//! Perception assembly for the Perception phase of the tick cycle.
//!
//! During the Perception phase, the engine builds a [`Perception`] payload
//! for each living agent. The perception contains everything the agent is
//! allowed to know: their own state, their surroundings (location resources,
//! structures, other agents), known routes, recent memories, available
//! actions, and system notifications.
//!
//! Agents can only see their current location (fog of war). Resource
//! quantities are fuzzified so agents cannot make perfectly optimal
//! decisions.
//!
//! Per `world-engine.md` section 2.2 and `agent-system.md` section 5.

use std::collections::BTreeMap;

use emergence_types::{
    AgentId, AgentState, Message, Perception, Personality, Resource, Season, SelfState, Sex,
    Surroundings, TimeOfDay, VisibleAgent, VisibleMessage, Weather,
};

use crate::fuzzy;

/// Default number of ticks before a message expires from the board.
pub const DEFAULT_MESSAGE_EXPIRY_TICKS: u64 = 10;

/// Context required to assemble a perception payload for one agent.
///
/// Assembled once per location per tick, then shared across all agents
/// at that location to avoid redundant computation.
#[derive(Debug, Clone)]
pub struct PerceptionContext {
    /// The current tick number.
    pub tick: u64,
    /// Current time of day.
    pub time_of_day: TimeOfDay,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
    /// Location name.
    pub location_name: String,
    /// Location description.
    pub location_description: String,
    /// Resources available at the location (exact quantities).
    pub location_resources: BTreeMap<Resource, u32>,
    /// Structures visible at the location (pre-formatted).
    pub structures_here: Vec<emergence_types::VisibleStructure>,
    /// Raw messages at the location (both direct and broadcast).
    /// Filtered per-agent during perception assembly.
    pub messages_here: Vec<Message>,
    /// Known routes from this location (pre-formatted).
    pub known_routes: Vec<emergence_types::KnownRoute>,
    /// Agent names by ID (for building the visible agents list).
    pub agent_names: BTreeMap<AgentId, String>,
    /// Agent sexes by ID (for building the visible agents list).
    pub agent_sexes: BTreeMap<AgentId, Sex>,
    /// Ticks until next season change (for notifications).
    pub ticks_until_season_change: u64,
    /// Number of ticks after which messages expire (default 10).
    pub message_expiry_ticks: u64,
}

/// Assemble a complete [`Perception`] payload for a single agent.
///
/// This is called once per agent during the Perception phase. The
/// `PerceptionContext` is shared across agents at the same location.
pub fn assemble_perception(
    agent_state: &AgentState,
    agent_name: &str,
    agent_sex: Sex,
    personality: Option<&Personality>,
    ctx: &PerceptionContext,
) -> Perception {
    // Build self-state
    let self_state = build_self_state(agent_state, agent_name, agent_sex, &ctx.location_name);

    // Build surroundings with fuzzy resource quantities
    let surroundings = build_surroundings(agent_state.agent_id, ctx);

    // Build available actions
    let available_actions = available_survival_actions(agent_state);

    // Build notifications
    let notifications = build_notifications(agent_state, ctx);

    // Recent memories (last 5 immediate-tier memories)
    let recent_memory = agent_state
        .memory
        .iter()
        .rev()
        .take(5)
        .map(|m| m.summary.clone())
        .collect();

    Perception {
        tick: ctx.tick,
        time_of_day: ctx.time_of_day,
        season: ctx.season,
        weather: ctx.weather,
        self_state,
        surroundings,
        known_routes: ctx.known_routes.clone(),
        recent_memory,
        available_actions,
        notifications,
        personality: personality.cloned(),
    }
}

/// Build the agent's self-state view.
fn build_self_state(agent: &AgentState, name: &str, sex: Sex, location_name: &str) -> SelfState {
    let total_weight = emergence_agents::inventory::total_weight(&agent.inventory).unwrap_or(0);
    let carry_load = format!("{total_weight}/{}", agent.carry_capacity);

    let known_skills: Vec<String> = agent
        .skills
        .iter()
        .map(|(skill, level)| format!("{skill} (lvl {level})"))
        .collect();

    SelfState {
        id: agent.agent_id,
        name: String::from(name),
        sex,
        age: agent.age,
        energy: agent.energy,
        health: agent.health,
        hunger: agent.hunger,
        thirst: agent.thirst,
        location_name: String::from(location_name),
        inventory: agent.inventory.clone(),
        carry_load,
        active_goals: agent.goals.clone(),
        known_skills,
    }
}

/// Build the surroundings visible to the agent.
///
/// Messages are filtered so that each agent only sees:
/// - Broadcast messages (visible to all)
/// - Direct messages where the agent is the recipient
///
/// Messages older than `message_expiry_ticks` are excluded.
fn build_surroundings(agent_id: AgentId, ctx: &PerceptionContext) -> Surroundings {
    // Fuzzify resource quantities
    let visible_resources: BTreeMap<Resource, String> = ctx
        .location_resources
        .iter()
        .map(|(resource, &qty)| (*resource, String::from(fuzzy::fuzzy_quantity(qty))))
        .collect();

    // Build visible agents list (excluding self)
    let agents_here: Vec<VisibleAgent> = ctx
        .agent_names
        .iter()
        .filter(|&(&id, _)| id != agent_id)
        .map(|(&id, name)| VisibleAgent {
            id,
            name: name.clone(),
            sex: ctx.agent_sexes.get(&id).copied().unwrap_or(Sex::Female),
            relationship: String::from("unknown"),
            activity: String::from("idle"),
        })
        .collect();

    // Filter messages: agent sees broadcasts + direct messages to them,
    // excluding expired messages.
    let messages_here: Vec<VisibleMessage> = filter_messages_for_agent(agent_id, ctx);

    Surroundings {
        location_description: ctx.location_description.clone(),
        visible_resources,
        structures_here: ctx.structures_here.clone(),
        agents_here,
        messages_here,
    }
}

/// Filter the raw message board for a specific agent.
///
/// Returns only non-expired messages that the agent should see:
/// - All broadcast messages
/// - Direct messages where the agent is the recipient
///
/// Messages are converted to [`VisibleMessage`] for the perception payload.
fn filter_messages_for_agent(agent_id: AgentId, ctx: &PerceptionContext) -> Vec<VisibleMessage> {
    let expiry_cutoff = ctx.tick.saturating_sub(ctx.message_expiry_ticks);

    ctx.messages_here
        .iter()
        .filter(|msg| {
            // Exclude expired messages
            if msg.tick < expiry_cutoff {
                return false;
            }
            // Agent sees broadcasts and messages addressed to them
            msg.is_broadcast || msg.recipient_id == Some(agent_id)
        })
        .map(|msg| VisibleMessage {
            from: msg.sender_name.clone(),
            tick: msg.tick,
            content: msg.content.clone(),
        })
        .collect()
}

/// Determine which survival actions the agent can currently perform.
fn available_survival_actions(agent: &AgentState) -> Vec<String> {
    let mut actions = Vec::new();

    // Traveling agents can only wait
    if agent.destination_id.is_some() {
        actions.push(String::from("wait (traveling)"));
        return actions;
    }

    // Gather is always an option if at a location
    actions.push(String::from("gather <resource>"));

    // Eat requires food in inventory
    let has_food = agent.inventory.keys().any(|r| {
        matches!(
            r,
            Resource::FoodBerry
                | Resource::FoodFish
                | Resource::FoodRoot
                | Resource::FoodMeat
                | Resource::FoodFarmed
                | Resource::FoodCooked
        )
    });
    if has_food {
        actions.push(String::from("eat <food_type>"));
    }

    // Drink is always listed (might be at location or in inventory)
    actions.push(String::from("drink"));

    // Rest is always available
    actions.push(String::from("rest"));

    // Move is available if not traveling
    actions.push(String::from("move <destination>"));

    // Communicate is available if other agents are present
    actions.push(String::from("communicate <agent> <message>"));

    // Broadcast is always available when at a location
    actions.push(String::from("broadcast <message>"));

    // No-action is always available
    actions.push(String::from("no_action"));

    actions
}

/// Build system notifications for the agent.
fn build_notifications(agent: &AgentState, ctx: &PerceptionContext) -> Vec<String> {
    let mut notes = Vec::new();

    // Low health warning
    if agent.health <= 30 {
        notes.push(format!(
            "WARNING: Health critically low ({}/100)",
            agent.health
        ));
    }

    // Low energy warning
    if agent.energy <= 20 {
        notes.push(format!(
            "WARNING: Energy critically low ({}/100)",
            agent.energy
        ));
    }

    // High hunger warning
    if agent.hunger >= 70 {
        notes.push(format!(
            "WARNING: Very hungry ({}/100). Eat soon or risk starvation.",
            agent.hunger
        ));
    }

    // High thirst warning
    if agent.thirst >= 70 {
        notes.push(format!(
            "WARNING: Very thirsty ({}/100). Drink soon or risk dehydration.",
            agent.thirst
        ));
    }

    // Approaching winter
    if ctx.season == Season::Autumn && ctx.ticks_until_season_change <= 10 {
        notes.push(format!(
            "Winter approaching in {} ticks. Stockpile food and find shelter.",
            ctx.ticks_until_season_change
        ));
    }

    // Storm warning
    if ctx.weather == Weather::Storm {
        notes.push(String::from(
            "STORM: Travel is blocked. Seek shelter.",
        ));
    }

    // Inventory full warning
    let current_load = emergence_agents::inventory::total_weight(&agent.inventory).unwrap_or(0);
    if agent.carry_capacity > 0 && current_load >= agent.carry_capacity {
        notes.push(String::from(
            "WARNING: Inventory full. Gathering will fail. Eat food, trade, or drop items to make room.",
        ));
    } else if agent.carry_capacity > 0 {
        let pct = (u64::from(current_load) * 100) / u64::from(agent.carry_capacity);
        if pct >= 90 {
            notes.push(format!(
                "Inventory nearly full ({current_load}/{}). Consider eating or dropping items.",
                agent.carry_capacity
            ));
        }
    }

    notes
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_types::*;

    use super::*;

    fn make_agent_state(agent_id: AgentId) -> AgentState {
        AgentState {
            agent_id,
            energy: 80,
            health: 100,
            hunger: 0,
            thirst: 0,
            age: 100,
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

    fn make_context(tick: u64) -> PerceptionContext {
        let mut resources = BTreeMap::new();
        resources.insert(Resource::Wood, 50);
        resources.insert(Resource::Water, 100);
        resources.insert(Resource::Stone, 3);

        PerceptionContext {
            tick,
            time_of_day: TimeOfDay::Morning,
            season: Season::Spring,
            weather: Weather::Clear,
            location_name: String::from("Green Meadow"),
            location_description: String::from("A lush green meadow with a stream."),
            location_resources: resources,
            structures_here: Vec::new(),
            messages_here: Vec::new(),
            known_routes: Vec::new(),
            agent_names: BTreeMap::new(),
            agent_sexes: BTreeMap::new(),
            ticks_until_season_change: 45,
            message_expiry_ticks: DEFAULT_MESSAGE_EXPIRY_TICKS,
        }
    }

    #[test]
    fn perception_contains_correct_tick() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let ctx = make_context(42);

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.tick, 42);
        assert_eq!(p.season, Season::Spring);
        assert_eq!(p.weather, Weather::Clear);
    }

    #[test]
    fn self_state_populated() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let ctx = make_context(1);

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.self_state.id, agent_id);
        assert_eq!(p.self_state.name, "Alpha");
        assert_eq!(p.self_state.energy, 80);
        assert_eq!(p.self_state.health, 100);
        assert_eq!(p.self_state.carry_load, "0/50");
    }

    #[test]
    fn resources_are_fuzzified() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let ctx = make_context(1);

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);

        // Wood=50 -> "abundant"
        assert_eq!(
            p.surroundings.visible_resources.get(&Resource::Wood),
            Some(&String::from("abundant"))
        );
        // Water=100 -> "plentiful"
        assert_eq!(
            p.surroundings.visible_resources.get(&Resource::Water),
            Some(&String::from("plentiful"))
        );
        // Stone=3 -> "scarce"
        assert_eq!(
            p.surroundings.visible_resources.get(&Resource::Stone),
            Some(&String::from("scarce"))
        );
    }

    #[test]
    fn other_agents_visible_self_excluded() {
        let agent_id = AgentId::new();
        let other_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(1);
        ctx.agent_names.insert(agent_id, String::from("Alpha"));
        ctx.agent_names.insert(other_id, String::from("Beta"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.surroundings.agents_here.len(), 1);
        assert_eq!(p.surroundings.agents_here.first().map(|a| a.name.as_str()), Some("Beta"));
    }

    #[test]
    fn traveling_agent_only_wait() {
        let agent_id = AgentId::new();
        let mut state = make_agent_state(agent_id);
        state.destination_id = Some(LocationId::new());
        state.travel_progress = 3;

        let actions = available_survival_actions(&state);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions.first().map(String::as_str), Some("wait (traveling)"));
    }

    #[test]
    fn stationary_agent_has_survival_actions() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);

        let actions = available_survival_actions(&state);
        assert!(actions.iter().any(|a| a.contains("gather")));
        assert!(actions.iter().any(|a| a.contains("drink")));
        assert!(actions.iter().any(|a| a.contains("rest")));
        assert!(actions.iter().any(|a| a.contains("move")));
        assert!(actions.iter().any(|a| a.contains("communicate")));
        assert!(actions.iter().any(|a| a.contains("broadcast")));
        assert!(actions.iter().any(|a| a.contains("no_action")));
    }

    #[test]
    fn eat_available_when_food_in_inventory() {
        let agent_id = AgentId::new();
        let mut state = make_agent_state(agent_id);
        state.inventory.insert(Resource::FoodBerry, 3);

        let actions = available_survival_actions(&state);
        assert!(actions.iter().any(|a| a.contains("eat")));
    }

    #[test]
    fn low_health_notification() {
        let agent_id = AgentId::new();
        let mut state = make_agent_state(agent_id);
        state.health = 20;
        let ctx = make_context(1);

        let notes = build_notifications(&state, &ctx);
        assert!(notes.iter().any(|n| n.contains("Health critically low")));
    }

    #[test]
    fn high_hunger_notification() {
        let agent_id = AgentId::new();
        let mut state = make_agent_state(agent_id);
        state.hunger = 80;
        let ctx = make_context(1);

        let notes = build_notifications(&state, &ctx);
        assert!(notes.iter().any(|n| n.contains("Very hungry")));
    }

    #[test]
    fn winter_approaching_notification() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(1);
        ctx.season = Season::Autumn;
        ctx.ticks_until_season_change = 5;

        let notes = build_notifications(&state, &ctx);
        assert!(notes.iter().any(|n| n.contains("Winter approaching")));
    }

    #[test]
    fn storm_notification() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(1);
        ctx.weather = Weather::Storm;

        let notes = build_notifications(&state, &ctx);
        assert!(notes.iter().any(|n| n.contains("STORM")));
    }

    #[test]
    fn no_notifications_when_healthy() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let ctx = make_context(1);

        let notes = build_notifications(&state, &ctx);
        assert!(notes.is_empty());
    }

    // -----------------------------------------------------------------------
    // Message filtering tests
    // -----------------------------------------------------------------------

    fn make_broadcast_message(sender_name: &str, tick: u64, content: &str) -> Message {
        Message {
            sender_id: AgentId::new(),
            sender_name: String::from(sender_name),
            recipient_id: None,
            content: String::from(content),
            tick,
            is_broadcast: true,
            location_id: LocationId::new(),
        }
    }

    fn make_direct_message(
        sender_name: &str,
        recipient_id: AgentId,
        tick: u64,
        content: &str,
    ) -> Message {
        Message {
            sender_id: AgentId::new(),
            sender_name: String::from(sender_name),
            recipient_id: Some(recipient_id),
            content: String::from(content),
            tick,
            is_broadcast: false,
            location_id: LocationId::new(),
        }
    }

    #[test]
    fn agent_sees_broadcast_messages() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(5);
        ctx.messages_here.push(make_broadcast_message("Dax", 4, "Hello everyone!"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.surroundings.messages_here.len(), 1);
        assert_eq!(p.surroundings.messages_here[0].from, "Dax");
        assert_eq!(p.surroundings.messages_here[0].content, "Hello everyone!");
    }

    #[test]
    fn agent_sees_direct_messages_to_them() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(5);
        ctx.messages_here.push(make_direct_message("Maren", agent_id, 4, "Hey Alpha!"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.surroundings.messages_here.len(), 1);
        assert_eq!(p.surroundings.messages_here[0].content, "Hey Alpha!");
    }

    #[test]
    fn agent_does_not_see_messages_for_others() {
        let agent_id = AgentId::new();
        let other_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(5);
        // Direct message to another agent
        ctx.messages_here.push(make_direct_message("Maren", other_id, 4, "Secret"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert!(p.surroundings.messages_here.is_empty());
    }

    #[test]
    fn agent_sees_mix_of_broadcast_and_direct() {
        let agent_id = AgentId::new();
        let other_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(5);
        ctx.messages_here.push(make_broadcast_message("Dax", 4, "Broadcast!"));
        ctx.messages_here.push(make_direct_message("Maren", agent_id, 4, "For you"));
        ctx.messages_here.push(make_direct_message("Zane", other_id, 4, "Not for you"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        // Should see broadcast + direct to self, not the one for other
        assert_eq!(p.surroundings.messages_here.len(), 2);
    }

    #[test]
    fn expired_messages_are_filtered_out() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(20); // Current tick = 20, expiry = 10
        // Message at tick 5 is expired (20 - 10 = 10, and 5 < 10)
        ctx.messages_here.push(make_broadcast_message("Dax", 5, "Old message"));
        // Message at tick 15 is not expired (15 >= 10)
        ctx.messages_here.push(make_broadcast_message("Maren", 15, "Recent message"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.surroundings.messages_here.len(), 1);
        assert_eq!(p.surroundings.messages_here[0].content, "Recent message");
    }

    #[test]
    fn message_at_expiry_boundary_is_included() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let mut ctx = make_context(20); // expiry cutoff = 20 - 10 = 10
        // Message at exactly tick 10 should be included (10 >= 10)
        ctx.messages_here.push(make_broadcast_message("Dax", 10, "Boundary message"));

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert_eq!(p.surroundings.messages_here.len(), 1);
    }

    #[test]
    fn no_messages_produces_empty_list() {
        let agent_id = AgentId::new();
        let state = make_agent_state(agent_id);
        let ctx = make_context(5);

        let p = assemble_perception(&state, "Alpha", Sex::Male, None, &ctx);
        assert!(p.surroundings.messages_here.is_empty());
    }
}
