//! Action request and result types for agent-to-engine communication.
//!
//! Defines the structs from `data-schemas.md` section 7: the request an agent
//! submits, the parameters for each action type, and the result returned after
//! resolution.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::enums::{ActionType, Resource, StructureType};
use crate::ids::{AgentId, GroupId, LocationId, RuleId, StructureId, TradeId};
use crate::structs::RejectionDetails;

// ---------------------------------------------------------------------------
// 7.2 ActionParameters
// ---------------------------------------------------------------------------

/// Action-specific parameters submitted alongside an [`ActionRequest`].
///
/// Each variant corresponds to one [`ActionType`] and carries the data
/// needed to validate and execute that action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum ActionParameters {
    /// Parameters for [`ActionType::Gather`].
    Gather {
        /// The resource to collect.
        resource: Resource,
    },
    /// Parameters for [`ActionType::Eat`].
    Eat {
        /// The food resource to consume from inventory.
        food_type: Resource,
    },
    /// Parameters for [`ActionType::Drink`].
    Drink,
    /// Parameters for [`ActionType::Rest`].
    Rest,
    /// Parameters for [`ActionType::Move`].
    Move {
        /// The destination location.
        destination: LocationId,
    },
    /// Parameters for [`ActionType::Build`].
    Build {
        /// The type of structure to build.
        structure_type: StructureType,
    },
    /// Parameters for [`ActionType::Repair`].
    Repair {
        /// The structure to repair.
        structure_id: StructureId,
    },
    /// Parameters for [`ActionType::Demolish`].
    Demolish {
        /// The structure to demolish.
        structure_id: StructureId,
    },
    /// Parameters for [`ActionType::ImproveRoute`].
    ImproveRoute {
        /// The route to improve (identified by destination).
        destination: LocationId,
    },
    /// Parameters for [`ActionType::Communicate`].
    Communicate {
        /// The agent to send a message to.
        target_agent: AgentId,
        /// Message content (max 500 chars).
        message: String,
    },
    /// Parameters for [`ActionType::Broadcast`].
    Broadcast {
        /// Message content (max 500 chars).
        message: String,
    },
    /// Parameters for [`ActionType::TradeOffer`].
    TradeOffer {
        /// The agent to propose a trade to.
        target_agent: AgentId,
        /// Resources offered.
        offer: BTreeMap<Resource, u32>,
        /// Resources requested in return.
        request: BTreeMap<Resource, u32>,
    },
    /// Parameters for [`ActionType::TradeAccept`].
    TradeAccept {
        /// The trade to accept.
        trade_id: TradeId,
    },
    /// Parameters for [`ActionType::TradeReject`].
    TradeReject {
        /// The trade to reject.
        trade_id: TradeId,
    },
    /// Parameters for [`ActionType::FormGroup`].
    FormGroup {
        /// Proposed group name.
        name: String,
        /// Agent IDs invited to join the group.
        invited_members: Vec<AgentId>,
    },
    /// Parameters for [`ActionType::Teach`].
    Teach {
        /// The agent to teach.
        target_agent: AgentId,
        /// The knowledge concept to teach.
        knowledge: String,
    },
    /// Parameters for [`ActionType::FarmPlant`].
    FarmPlant,
    /// Parameters for [`ActionType::FarmHarvest`].
    FarmHarvest,
    /// Parameters for [`ActionType::Craft`].
    Craft {
        /// What to craft (resource output).
        output: Resource,
    },
    /// Parameters for [`ActionType::Mine`].
    Mine,
    /// Parameters for [`ActionType::Smelt`].
    Smelt,
    /// Parameters for [`ActionType::Write`].
    Write {
        /// Knowledge to persist to the library.
        knowledge: String,
    },
    /// Parameters for [`ActionType::Read`].
    Read {
        /// Knowledge to retrieve from the library.
        knowledge: String,
    },
    /// Parameters for [`ActionType::Claim`].
    Claim {
        /// The structure to claim.
        structure_id: StructureId,
    },
    /// Parameters for [`ActionType::Legislate`].
    Legislate {
        /// Display name for the rule or law.
        rule_name: String,
        /// Description of what the rule mandates or prohibits.
        rule_description: String,
        /// The group this rule applies to.
        group_id: GroupId,
    },
    /// Parameters for [`ActionType::Enforce`].
    Enforce {
        /// The agent to enforce against.
        target_agent: AgentId,
        /// The rule being enforced (by ID).
        rule_id: RuleId,
        /// Description of the consequence being applied.
        consequence: String,
    },
    /// Parameters for [`ActionType::Reproduce`].
    Reproduce {
        /// The partner agent.
        partner_agent: AgentId,
    },
    /// Parameters for [`ActionType::NoAction`].
    NoAction,
}

// ---------------------------------------------------------------------------
// 7.1 ActionRequest
// ---------------------------------------------------------------------------

/// An action submitted by an agent to the World Engine for validation
/// and execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActionRequest {
    /// The agent submitting this action.
    pub agent_id: AgentId,
    /// The current tick number.
    pub tick: u64,
    /// The type of action being taken.
    pub action_type: ActionType,
    /// Action-specific data.
    pub parameters: ActionParameters,
    /// Real-world submission timestamp.
    pub submitted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// 7.3 ActionOutcome
// ---------------------------------------------------------------------------

/// The outcome of a successfully executed action.
///
/// This is a generic container -- the actual payload depends on the action
/// type and is stored as a JSON value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActionOutcome {
    /// Resources gained or spent as a result of the action.
    pub resource_changes: BTreeMap<Resource, i64>,
    /// Energy spent on the action.
    pub energy_spent: u32,
    /// Skill experience gained.
    pub skill_xp: BTreeMap<String, u32>,
    /// Additional outcome-specific data.
    pub details: serde_json::Value,
}

// ---------------------------------------------------------------------------
// 7.3 ActionResult
// ---------------------------------------------------------------------------

/// The result returned to an agent after the resolution phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActionResult {
    /// The tick of the action.
    pub tick: u64,
    /// The agent who acted.
    pub agent_id: AgentId,
    /// The action that was attempted.
    pub action_type: ActionType,
    /// Whether the action succeeded.
    pub success: bool,
    /// Success details (present only if `success` is true).
    pub outcome: Option<ActionOutcome>,
    /// Failure details (present only if `success` is false).
    pub rejection: Option<RejectionDetails>,
    /// Observable consequences of the action.
    pub side_effects: Vec<String>,
}
