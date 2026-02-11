//! Perception payload types delivered to agents each tick.
//!
//! The perception is the **only** information an agent receives about the
//! world. If something is not in the perception, the agent does not know
//! about it. This enforces fog of war.
//!
//! Defined in `data-schemas.md` section 8.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::enums::{Resource, Season, TimeOfDay, Weather};
use crate::ids::AgentId;
use crate::structs::{Sex, VisibleMessage, VisibleStructure};

// ---------------------------------------------------------------------------
// 8.1 Perception
// ---------------------------------------------------------------------------

/// The complete perception payload delivered to an agent at the start of
/// each tick. This is everything the agent can "see."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Perception {
    /// Current tick number.
    pub tick: u64,
    /// Current time of day.
    pub time_of_day: TimeOfDay,
    /// Current season.
    pub season: Season,
    /// Current weather.
    pub weather: Weather,
    /// The agent's own state summary.
    pub self_state: SelfState,
    /// What the agent can see at their current location.
    pub surroundings: Surroundings,
    /// Routes the agent knows about from this location.
    pub known_routes: Vec<KnownRoute>,
    /// Relevant memories from the agent's memory system.
    pub recent_memory: Vec<String>,
    /// Actions the agent can currently perform.
    pub available_actions: Vec<String>,
    /// System notifications (approaching winter, low health, etc.).
    pub notifications: Vec<String>,
}

// ---------------------------------------------------------------------------
// 8.2 SelfState
// ---------------------------------------------------------------------------

/// The agent's own state as presented in perception.
///
/// This is a simplified, agent-facing view of the full [`AgentState`].
///
/// [`AgentState`]: crate::structs::AgentState
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SelfState {
    /// The agent's identifier.
    pub id: AgentId,
    /// The agent's name.
    pub name: String,
    /// The agent's biological sex.
    pub sex: Sex,
    /// Current age in ticks.
    pub age: u32,
    /// Current energy (0--100).
    pub energy: u32,
    /// Current health (0--100).
    pub health: u32,
    /// Current hunger (0--100).
    pub hunger: u32,
    /// Current thirst (0--100).
    pub thirst: u32,
    /// Name of the agent's current location.
    pub location_name: String,
    /// Inventory contents.
    pub inventory: BTreeMap<Resource, u32>,
    /// Carry load as a formatted string (e.g. "26/50").
    pub carry_load: String,
    /// Current active goals.
    pub active_goals: Vec<String>,
    /// Known skills with level (e.g. "gathering (lvl 4)").
    pub known_skills: Vec<String>,
}

// ---------------------------------------------------------------------------
// 8.3 Surroundings
// ---------------------------------------------------------------------------

/// What the agent can see at their current location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Surroundings {
    /// Narrative description of the location.
    pub location_description: String,
    /// Fuzzy resource quantities (e.g. "abundant", "scarce").
    pub visible_resources: BTreeMap<Resource, String>,
    /// Structures present at this location.
    pub structures_here: Vec<VisibleStructure>,
    /// Other agents present at this location.
    pub agents_here: Vec<VisibleAgent>,
    /// Broadcast messages posted at this location.
    pub messages_here: Vec<VisibleMessage>,
}

// ---------------------------------------------------------------------------
// 8.4 VisibleAgent
// ---------------------------------------------------------------------------

/// Another agent as seen in the perception's surroundings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct VisibleAgent {
    /// The other agent's name.
    pub name: String,
    /// The other agent's biological sex.
    pub sex: Sex,
    /// Relationship description (e.g. "friendly (0.7)").
    pub relationship: String,
    /// What the agent appears to be doing.
    pub activity: String,
}

// ---------------------------------------------------------------------------
// 8.5 KnownRoute
// ---------------------------------------------------------------------------

/// A route the agent knows about from their current location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct KnownRoute {
    /// Destination location name.
    pub destination: String,
    /// Travel cost as a formatted string (e.g. "3 ticks").
    pub cost: String,
    /// Path quality description (e.g. "dirt trail").
    pub path_type: String,
}
