//! Shared type definitions for the Emergence simulation.
//!
//! This crate is the single source of truth for all types used across the
//! Emergence workspace. Types defined here flow downstream to `TypeScript`
//! via `ts-rs` for the Observer Dashboard.
//!
//! # Modules
//!
//! - [`ids`] -- Type-safe UUID wrappers for all entity identifiers
//! - [`enums`] -- Enumeration types (resources, actions, events, environment)
//! - [`structs`] -- Core entity structs (agents, locations, structures, ledger)
//! - [`actions`] -- Action request/result types for agent-engine communication
//! - [`perception`] -- Perception payload delivered to agents each tick

pub mod actions;
pub mod enums;
pub mod ids;
pub mod perception;
pub mod structs;

// Re-export all public types at crate root for convenience.
pub use actions::{ActionOutcome, ActionParameters, ActionRequest, ActionResult};
pub use enums::{
    ActionType, EntityType, Era, EventType, LedgerEntryType, MemoryTier, PathType, RejectionReason,
    Resource, Season, StructureCategory, StructureType, TimeOfDay, Weather,
};
pub use ids::{
    AgentId, EventId, GroupId, LedgerEntryId, LocationId, RouteId, RuleId, StructureId, TradeId,
};
pub use perception::{KnownRoute, Perception, SelfState, Surroundings, VisibleAgent};
pub use structs::{
    AccessControlList, ActionRejectedDetails, ActionSucceededDetails, Agent, AgentDiedDetails,
    AgentState, AgentStateSnapshot, EconomyStats, EnforcementAppliedDetails, Event, Group,
    GroupFormedDetails, InteractionCause, KnowledgeDiscoveredDetails, KnowledgeTaughtDetails,
    LedgerEntry, Location, LocationEffects, MemoryEntry, Message, PendingTrade, Personality,
    PopulationStats, RejectionDetails, RelationshipChangedDetails, ResourceGatheredDetails,
    ResourceNode, Route, RouteDegradedDetails, RouteImprovedDetails, Rule, RuleCreatedDetails,
    Structure, StructureBlueprint, StructureBuiltDetails, StructureClaimedDetails,
    StructureDestroyedDetails, StructureProperties, StructureRepairedDetails,
    TradeCompletedDetails, TradeFailReason, TradeFailedDetails, VisibleMessage, VisibleStructure,
    WorldContext, WorldSnapshot, memory_types,
};

#[cfg(test)]
mod tests {
    //! Integration tests for type exports and `TypeScript` binding generation.

    #[test]
    fn export_bindings() {
        // ts-rs generates TypeScript bindings when types with
        // #[ts(export)] are used. Importing them here triggers generation.
        // The actual files are written to the `bindings/` directory
        // relative to the crate root.
        use ts_rs::TS;

        // IDs
        let _ = crate::ids::AgentId::export_all();
        let _ = crate::ids::LocationId::export_all();
        let _ = crate::ids::StructureId::export_all();
        let _ = crate::ids::RouteId::export_all();
        let _ = crate::ids::EventId::export_all();
        let _ = crate::ids::TradeId::export_all();
        let _ = crate::ids::GroupId::export_all();
        let _ = crate::ids::LedgerEntryId::export_all();
        let _ = crate::ids::RuleId::export_all();

        // Enums
        let _ = crate::enums::Resource::export_all();
        let _ = crate::enums::StructureType::export_all();
        let _ = crate::enums::ActionType::export_all();
        let _ = crate::enums::EventType::export_all();
        let _ = crate::enums::RejectionReason::export_all();
        let _ = crate::enums::Season::export_all();
        let _ = crate::enums::Weather::export_all();
        let _ = crate::enums::PathType::export_all();
        let _ = crate::enums::TimeOfDay::export_all();
        let _ = crate::enums::Era::export_all();
        let _ = crate::enums::LedgerEntryType::export_all();
        let _ = crate::enums::EntityType::export_all();
        let _ = crate::enums::MemoryTier::export_all();
        let _ = crate::enums::StructureCategory::export_all();

        // Structs
        let _ = crate::structs::Personality::export_all();
        let _ = crate::structs::MemoryEntry::export_all();
        let _ = crate::structs::ResourceNode::export_all();
        let _ = crate::structs::AccessControlList::export_all();
        let _ = crate::structs::StructureProperties::export_all();
        let _ = crate::structs::WorldContext::export_all();
        let _ = crate::structs::AgentStateSnapshot::export_all();
        let _ = crate::structs::Event::export_all();
        let _ = crate::structs::LedgerEntry::export_all();
        let _ = crate::structs::Agent::export_all();
        let _ = crate::structs::AgentState::export_all();
        let _ = crate::structs::Location::export_all();
        let _ = crate::structs::Route::export_all();
        let _ = crate::structs::Structure::export_all();
        let _ = crate::structs::WorldSnapshot::export_all();
        let _ = crate::structs::PopulationStats::export_all();
        let _ = crate::structs::EconomyStats::export_all();
        let _ = crate::structs::ActionSucceededDetails::export_all();
        let _ = crate::structs::ActionRejectedDetails::export_all();
        let _ = crate::structs::ResourceGatheredDetails::export_all();
        let _ = crate::structs::TradeCompletedDetails::export_all();
        let _ = crate::structs::KnowledgeDiscoveredDetails::export_all();
        let _ = crate::structs::KnowledgeTaughtDetails::export_all();
        let _ = crate::structs::AgentDiedDetails::export_all();
        let _ = crate::structs::VisibleStructure::export_all();
        let _ = crate::structs::VisibleMessage::export_all();
        let _ = crate::structs::Message::export_all();
        let _ = crate::structs::RejectionDetails::export_all();
        let _ = crate::structs::PendingTrade::export_all();
        let _ = crate::structs::TradeFailReason::export_all();
        let _ = crate::structs::TradeFailedDetails::export_all();
        let _ = crate::structs::InteractionCause::export_all();
        let _ = crate::structs::RelationshipChangedDetails::export_all();
        let _ = crate::structs::GroupFormedDetails::export_all();
        let _ = crate::structs::Group::export_all();
        let _ = crate::structs::StructureBlueprint::export_all();
        let _ = crate::structs::LocationEffects::export_all();
        let _ = crate::structs::StructureBuiltDetails::export_all();
        let _ = crate::structs::StructureRepairedDetails::export_all();
        let _ = crate::structs::StructureDestroyedDetails::export_all();
        let _ = crate::structs::RouteImprovedDetails::export_all();
        let _ = crate::structs::RouteDegradedDetails::export_all();
        let _ = crate::structs::Rule::export_all();
        let _ = crate::structs::StructureClaimedDetails::export_all();
        let _ = crate::structs::RuleCreatedDetails::export_all();
        let _ = crate::structs::EnforcementAppliedDetails::export_all();

        // Actions
        let _ = crate::actions::ActionParameters::export_all();
        let _ = crate::actions::ActionRequest::export_all();
        let _ = crate::actions::ActionOutcome::export_all();
        let _ = crate::actions::ActionResult::export_all();

        // Perception
        let _ = crate::perception::Perception::export_all();
        let _ = crate::perception::SelfState::export_all();
        let _ = crate::perception::Surroundings::export_all();
        let _ = crate::perception::VisibleAgent::export_all();
        let _ = crate::perception::KnownRoute::export_all();
    }
}
