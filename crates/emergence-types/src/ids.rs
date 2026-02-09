//! Type-safe identifier wrappers around [`Uuid`].
//!
//! Every entity in the simulation has a strongly-typed ID to prevent
//! accidental mixing of identifiers at compile time. All IDs use UUID v7
//! (time-ordered) for efficient database indexing.
//!
//! `PostgreSQL` 18 generates UUIDs via native `DEFAULT uuidv7()` for inserts.
//! The `new()` constructors here exist for cases where app-side generation
//! is needed (e.g. tests, seed data).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Generates a newtype wrapper around [`Uuid`] with standard derives.
macro_rules! define_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
        #[ts(export, export_to = "bindings/")]
        pub struct $name(pub Uuid);

        impl $name {
            /// Create a new identifier using UUID v7 (time-ordered).
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Return the inner [`Uuid`] value.
            pub const fn into_inner(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id! {
    /// Unique identifier for an agent in the simulation.
    AgentId
}

define_id! {
    /// Unique identifier for a location (node in the world graph).
    LocationId
}

define_id! {
    /// Unique identifier for a structure built at a location.
    StructureId
}

define_id! {
    /// Unique identifier for a route (edge in the world graph).
    RouteId
}

define_id! {
    /// Unique identifier for an event in the event store.
    EventId
}

define_id! {
    /// Unique identifier for a trade between two agents.
    TradeId
}

define_id! {
    /// Unique identifier for a social group of agents.
    GroupId
}

define_id! {
    /// Unique identifier for a ledger entry (resource transfer record).
    LedgerEntryId
}

define_id! {
    /// Unique identifier for a governance rule created via the `Legislate` action.
    RuleId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_types() {
        let agent = AgentId::new();
        let location = LocationId::new();
        // These are different types -- the compiler enforces no mixing.
        assert_ne!(agent.into_inner(), Uuid::nil());
        assert_ne!(location.into_inner(), Uuid::nil());
    }

    #[test]
    fn id_roundtrip_serde() {
        let original = AgentId::new();
        let json = serde_json::to_string(&original).ok();
        assert!(json.is_some());
        let restored: Result<AgentId, _> = serde_json::from_str(
            json.as_deref().unwrap_or(""),
        );
        assert!(restored.is_ok());
    }

    #[test]
    fn id_display_matches_uuid() {
        let id = AgentId::new();
        assert_eq!(id.to_string(), id.into_inner().to_string());
    }
}
