//! Social construct data model for the Emergence simulation.
//!
//! Social constructs are emergent institutions that arise from agent
//! interactions: religions, governments, economic systems, family units,
//! and cultural traditions. This module implements task 6.4.1 from the
//! build plan.
//!
//! # Architecture
//!
//! A `SocialConstruct` is a named entity with a category, a set of
//! adherent agents, flexible metadata properties, and an append-only
//! evolution history. The `ConstructRegistry` holds all constructs and
//! provides efficient lookup by ID, category, and agent membership.
//!
//! # Category Alignment
//!
//! The `SocialConstructCategory` enum mirrors the `PostgreSQL` enum
//! `social_construct_category` defined in migration `0006`. The Rust
//! enum is the source of truth for the application layer; the DB enum
//! is the source of truth for storage.
//!
//! # Invariants
//!
//! - A construct cannot have adherents after it is disbanded.
//! - Merge always absorbs the smaller construct into the larger one.
//! - Schism produces a new construct with a subset of members removed
//!   from the original.
//! - Every mutation is recorded in the evolution history.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::AgentId;

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// SocialConstructCategory
// ---------------------------------------------------------------------------

/// The category of a social construct, matching the DB enum in migration 0006.
///
/// Categories determine how the construct is classified for analytics,
/// perception assembly, and era detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocialConstructCategory {
    /// A belief system with spiritual or supernatural elements.
    Religion,
    /// A system of rules, leadership, and authority.
    Governance,
    /// A system of production, trade, and resource distribution.
    Economic,
    /// A kinship or partnership-based social unit.
    Family,
    /// Traditions, art, language, or shared identity.
    Cultural,
}

// ---------------------------------------------------------------------------
// ConstructEventType
// ---------------------------------------------------------------------------

/// The type of change recorded in a construct's evolution history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstructEventType {
    /// The construct was created.
    Founded,
    /// An agent joined the construct.
    MemberJoined,
    /// An agent left the construct.
    MemberLeft,
    /// A metadata property was added or changed.
    PropertyChanged,
    /// The leader of the construct changed.
    LeaderChanged,
    /// The construct split into two.
    Schism,
    /// Two constructs were merged into one.
    Merged,
    /// The construct was disbanded.
    Disbanded,
}

// ---------------------------------------------------------------------------
// ConstructEvent
// ---------------------------------------------------------------------------

/// A single entry in a construct's evolution history.
///
/// Every mutation to a construct produces a `ConstructEvent` so that the
/// full history can be replayed or inspected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructEvent {
    /// The tick when this event occurred.
    pub tick: u64,
    /// The type of change.
    pub event_type: ConstructEventType,
    /// Human-readable description of the change.
    pub description: String,
    /// The agent who caused the change, if applicable.
    pub agent_id: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// SocialConstruct
// ---------------------------------------------------------------------------

/// An emergent social institution in the simulation.
///
/// Social constructs are created by agents (or detected by the belief
/// detection pipeline) and evolve over time as agents join, leave, and
/// modify properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialConstruct {
    /// Unique construct identifier.
    pub id: Uuid,
    /// Display name of the construct.
    pub name: String,
    /// Category classification.
    pub category: SocialConstructCategory,
    /// The agent who founded this construct, if known.
    pub founded_by: Option<AgentId>,
    /// The tick when this construct was founded.
    pub founded_at_tick: u64,
    /// The tick when this construct was disbanded, if applicable.
    pub disbanded_at_tick: Option<u64>,
    /// Current adherent agent IDs.
    pub adherent_ids: HashSet<AgentId>,
    /// Flexible metadata (tenets, laws, currency name, etc.).
    pub properties: HashMap<String, String>,
    /// Append-only log of changes to this construct.
    pub evolution_history: Vec<ConstructEvent>,
}

impl SocialConstruct {
    /// Check whether this construct is currently active (not disbanded).
    pub const fn is_active(&self) -> bool {
        self.disbanded_at_tick.is_none()
    }

    /// Return the number of current adherents.
    pub fn adherent_count(&self) -> usize {
        self.adherent_ids.len()
    }
}

// ---------------------------------------------------------------------------
// ConstructRegistry
// ---------------------------------------------------------------------------

/// Registry holding all social constructs in the simulation.
///
/// Provides efficient lookup by ID, category, and agent membership.
/// All mutations record events in the affected construct's evolution
/// history.
#[derive(Debug, Clone)]
pub struct ConstructRegistry {
    /// All constructs, keyed by their unique ID.
    constructs: BTreeMap<Uuid, SocialConstruct>,
}

impl ConstructRegistry {
    /// Create a new empty registry.
    pub const fn new() -> Self {
        Self {
            constructs: BTreeMap::new(),
        }
    }

    /// Register a new social construct.
    ///
    /// Creates the construct, adds the founder as the first adherent
    /// (if provided), records a `Founded` event, and stores it.
    ///
    /// Returns the construct ID.
    pub fn register_construct(
        &mut self,
        name: String,
        category: SocialConstructCategory,
        founded_by: Option<AgentId>,
        founded_at_tick: u64,
        initial_properties: HashMap<String, String>,
    ) -> Uuid {
        let id = Uuid::now_v7();

        let mut adherent_ids = HashSet::new();
        if let Some(founder) = founded_by {
            adherent_ids.insert(founder);
        }

        let founder_desc = founded_by.map_or_else(
            || String::from("(system)"),
            |a| a.to_string(),
        );

        let founding_event = ConstructEvent {
            tick: founded_at_tick,
            event_type: ConstructEventType::Founded,
            description: format!("Founded by {founder_desc}"),
            agent_id: founded_by,
        };

        let construct = SocialConstruct {
            id,
            name,
            category,
            founded_by,
            founded_at_tick,
            disbanded_at_tick: None,
            adherent_ids,
            properties: initial_properties,
            evolution_history: vec![founding_event],
        };

        self.constructs.insert(id, construct);
        id
    }

    /// Disband a construct: mark it as disbanded and remove all adherents.
    ///
    /// Returns an error if the construct is not found or is already disbanded.
    pub fn disband_construct(
        &mut self,
        construct_id: Uuid,
        tick: u64,
        agent_id: Option<AgentId>,
    ) -> Result<(), AgentError> {
        let construct = self.constructs.get_mut(&construct_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("construct {construct_id} not found"),
            }
        })?;

        if construct.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("construct {construct_id} is already disbanded"),
            });
        }

        construct.disbanded_at_tick = Some(tick);
        construct.adherent_ids.clear();

        construct.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::Disbanded,
            description: String::from("Construct disbanded"),
            agent_id,
        });

        Ok(())
    }

    /// Add a member to a construct.
    ///
    /// Returns an error if the construct is not found or is disbanded.
    pub fn add_member(
        &mut self,
        construct_id: Uuid,
        agent_id: AgentId,
        tick: u64,
    ) -> Result<(), AgentError> {
        let construct = self.constructs.get_mut(&construct_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("construct {construct_id} not found"),
            }
        })?;

        if construct.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("cannot join disbanded construct {construct_id}"),
            });
        }

        construct.adherent_ids.insert(agent_id);

        construct.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::MemberJoined,
            description: format!("Agent {agent_id} joined"),
            agent_id: Some(agent_id),
        });

        Ok(())
    }

    /// Remove a member from a construct.
    ///
    /// Returns an error if the construct is not found. Silently succeeds
    /// if the agent was not a member (idempotent removal).
    pub fn remove_member(
        &mut self,
        construct_id: Uuid,
        agent_id: AgentId,
        tick: u64,
    ) -> Result<(), AgentError> {
        let construct = self.constructs.get_mut(&construct_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("construct {construct_id} not found"),
            }
        })?;

        construct.adherent_ids.remove(&agent_id);

        construct.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::MemberLeft,
            description: format!("Agent {agent_id} left"),
            agent_id: Some(agent_id),
        });

        Ok(())
    }

    /// Update a property on a construct (upsert semantics).
    ///
    /// Logs the change to the evolution history.
    /// Returns an error if the construct is not found or is disbanded.
    pub fn update_property(
        &mut self,
        construct_id: Uuid,
        key: &str,
        value: &str,
        tick: u64,
        agent_id: Option<AgentId>,
    ) -> Result<(), AgentError> {
        let construct = self.constructs.get_mut(&construct_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("construct {construct_id} not found"),
            }
        })?;

        if construct.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("cannot update disbanded construct {construct_id}"),
            });
        }

        construct
            .properties
            .insert(String::from(key), String::from(value));

        construct.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::PropertyChanged,
            description: format!("Property '{key}' set to '{value}'"),
            agent_id,
        });

        Ok(())
    }

    /// Get all constructs matching a given category.
    pub fn get_by_category(&self, category: SocialConstructCategory) -> Vec<&SocialConstruct> {
        self.constructs
            .values()
            .filter(|c| c.category == category)
            .collect()
    }

    /// Get all constructs an agent belongs to.
    pub fn get_agent_constructs(&self, agent_id: AgentId) -> Vec<&SocialConstruct> {
        self.constructs
            .values()
            .filter(|c| c.adherent_ids.contains(&agent_id))
            .collect()
    }

    /// Get a construct by ID.
    pub fn get_construct(&self, construct_id: Uuid) -> Option<&SocialConstruct> {
        self.constructs.get(&construct_id)
    }

    /// Count the number of active (non-disbanded) constructs.
    pub fn active_count(&self) -> usize {
        self.constructs.values().filter(|c| c.is_active()).count()
    }

    /// Merge two constructs into one.
    ///
    /// The larger construct (by adherent count) absorbs the smaller one.
    /// If they have the same size, the first argument (`construct_a`) is
    /// the absorber. The absorbed construct is disbanded.
    ///
    /// Returns the ID of the surviving construct.
    pub fn merge_constructs(
        &mut self,
        construct_a: Uuid,
        construct_b: Uuid,
        tick: u64,
        agent_id: Option<AgentId>,
    ) -> Result<Uuid, AgentError> {
        if construct_a == construct_b {
            return Err(AgentError::GovernanceFailed {
                reason: String::from("cannot merge a construct with itself"),
            });
        }

        // Read both constructs to determine which is larger.
        let count_a = self
            .constructs
            .get(&construct_a)
            .ok_or_else(|| AgentError::GovernanceFailed {
                reason: format!("construct {construct_a} not found"),
            })?
            .adherent_count();

        let count_b = self
            .constructs
            .get(&construct_b)
            .ok_or_else(|| AgentError::GovernanceFailed {
                reason: format!("construct {construct_b} not found"),
            })?
            .adherent_count();

        // Determine survivor (larger) and victim (smaller).
        let (survivor_id, victim_id) = if count_a >= count_b {
            (construct_a, construct_b)
        } else {
            (construct_b, construct_a)
        };

        // Collect members and name from the construct being dissolved.
        let dissolving = self
            .constructs
            .get(&victim_id)
            .ok_or_else(|| AgentError::GovernanceFailed {
                reason: format!("construct {victim_id} not found"),
            })?;

        if dissolving.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("construct {victim_id} is already disbanded"),
            });
        }

        let merged_members: Vec<AgentId> = dissolving.adherent_ids.iter().copied().collect();
        let merged_name = dissolving.name.clone();

        // Disband the dissolved construct.
        self.disband_construct(victim_id, tick, agent_id)?;

        // Add dissolved members to survivor and record the merge event.
        let survivor = self
            .constructs
            .get_mut(&survivor_id)
            .ok_or_else(|| AgentError::GovernanceFailed {
                reason: format!("construct {survivor_id} not found"),
            })?;

        if survivor.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("construct {survivor_id} is already disbanded"),
            });
        }

        for member in &merged_members {
            survivor.adherent_ids.insert(*member);
        }

        survivor.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::Merged,
            description: format!("Merged with '{merged_name}' ({victim_id})"),
            agent_id,
        });

        Ok(survivor_id)
    }

    /// Split a construct into two via schism.
    ///
    /// The `splinter_members` are removed from the original construct
    /// and placed into a new construct with the given name.
    ///
    /// Returns the ID of the new splinter construct.
    pub fn schism(
        &mut self,
        original_id: Uuid,
        splinter_name: String,
        splinter_members: &HashSet<AgentId>,
        tick: u64,
        agent_id: Option<AgentId>,
    ) -> Result<Uuid, AgentError> {
        let original = self.constructs.get_mut(&original_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("construct {original_id} not found"),
            }
        })?;

        if original.disbanded_at_tick.is_some() {
            return Err(AgentError::GovernanceFailed {
                reason: format!("cannot split disbanded construct {original_id}"),
            });
        }

        if splinter_members.is_empty() {
            return Err(AgentError::GovernanceFailed {
                reason: String::from("schism requires at least one splinter member"),
            });
        }

        // Remove splinter members from original.
        for member in splinter_members {
            original.adherent_ids.remove(member);
        }

        let category = original.category;

        original.evolution_history.push(ConstructEvent {
            tick,
            event_type: ConstructEventType::Schism,
            description: format!("Schism: '{splinter_name}' split off"),
            agent_id,
        });

        // Create the new splinter construct.
        let splinter_id = Uuid::now_v7();

        let founding_event = ConstructEvent {
            tick,
            event_type: ConstructEventType::Founded,
            description: format!("Founded via schism from construct {original_id}"),
            agent_id,
        };

        let splinter = SocialConstruct {
            id: splinter_id,
            name: splinter_name,
            category,
            founded_by: agent_id,
            founded_at_tick: tick,
            disbanded_at_tick: None,
            adherent_ids: splinter_members.clone(),
            properties: HashMap::new(),
            evolution_history: vec![founding_event],
        };

        self.constructs.insert(splinter_id, splinter);

        Ok(splinter_id)
    }
}

impl Default for ConstructRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use emergence_types::AgentId;

    use super::*;

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn make_registry_with_construct(
        name: &str,
        category: SocialConstructCategory,
        founder: AgentId,
        tick: u64,
    ) -> (ConstructRegistry, Uuid) {
        let mut registry = ConstructRegistry::new();
        let id = registry.register_construct(
            String::from(name),
            category,
            Some(founder),
            tick,
            HashMap::new(),
        );
        (registry, id)
    }

    // -----------------------------------------------------------------------
    // 1. Creation
    // -----------------------------------------------------------------------

    #[test]
    fn register_construct_creates_with_founder() {
        let founder = AgentId::new();
        let (registry, id) = make_registry_with_construct(
            "Sun Worshippers",
            SocialConstructCategory::Religion,
            founder,
            10,
        );

        let construct = registry.get_construct(id);
        assert!(construct.is_some());

        let c = construct.unwrap_or_else(|| {
            // Return a reference to a static -- never reached.
            static FALLBACK: std::sync::LazyLock<SocialConstruct> =
                std::sync::LazyLock::new(|| SocialConstruct {
                    id: Uuid::nil(),
                    name: String::new(),
                    category: SocialConstructCategory::Religion,
                    founded_by: None,
                    founded_at_tick: 0,
                    disbanded_at_tick: None,
                    adherent_ids: HashSet::new(),
                    properties: HashMap::new(),
                    evolution_history: Vec::new(),
                });
            &FALLBACK
        });

        assert_eq!(c.name, "Sun Worshippers");
        assert_eq!(c.category, SocialConstructCategory::Religion);
        assert_eq!(c.founded_by, Some(founder));
        assert_eq!(c.founded_at_tick, 10);
        assert!(c.is_active());
        assert!(c.adherent_ids.contains(&founder));
        assert_eq!(c.adherent_count(), 1);
        assert_eq!(c.evolution_history.len(), 1);
    }

    #[test]
    fn register_construct_without_founder() {
        let mut registry = ConstructRegistry::new();
        let id = registry.register_construct(
            String::from("Market System"),
            SocialConstructCategory::Economic,
            None,
            5,
            HashMap::new(),
        );

        let c = registry.get_construct(id);
        assert!(c.is_some());
        let c = c.unwrap_or_else(|| {
            static FALLBACK: std::sync::LazyLock<SocialConstruct> =
                std::sync::LazyLock::new(|| SocialConstruct {
                    id: Uuid::nil(),
                    name: String::new(),
                    category: SocialConstructCategory::Economic,
                    founded_by: None,
                    founded_at_tick: 0,
                    disbanded_at_tick: None,
                    adherent_ids: HashSet::new(),
                    properties: HashMap::new(),
                    evolution_history: Vec::new(),
                });
            &FALLBACK
        });
        assert_eq!(c.founded_by, None);
        assert_eq!(c.adherent_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 2. Membership
    // -----------------------------------------------------------------------

    #[test]
    fn add_member_success() {
        let founder = AgentId::new();
        let new_member = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Council",
            SocialConstructCategory::Governance,
            founder,
            1,
        );

        let result = registry.add_member(id, new_member, 5);
        assert!(result.is_ok());

        let c = registry.get_construct(id);
        assert!(c.is_some_and(|c| c.adherent_ids.contains(&new_member)));
        assert!(c.is_some_and(|c| c.adherent_count() == 2));
    }

    #[test]
    fn add_member_to_disbanded_fails() {
        let founder = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Dead Cult",
            SocialConstructCategory::Religion,
            founder,
            1,
        );
        let _ = registry.disband_construct(id, 10, None);

        let result = registry.add_member(id, AgentId::new(), 15);
        assert!(result.is_err());
    }

    #[test]
    fn remove_member_success() {
        let founder = AgentId::new();
        let member = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Family",
            SocialConstructCategory::Family,
            founder,
            1,
        );
        let _ = registry.add_member(id, member, 2);

        let result = registry.remove_member(id, member, 5);
        assert!(result.is_ok());

        let c = registry.get_construct(id);
        assert!(c.is_some_and(|c| !c.adherent_ids.contains(&member)));
    }

    #[test]
    fn remove_member_not_found_construct() {
        let mut registry = ConstructRegistry::new();
        let result = registry.remove_member(Uuid::nil(), AgentId::new(), 1);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 3. Property changes
    // -----------------------------------------------------------------------

    #[test]
    fn update_property_success() {
        let founder = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Monotheism",
            SocialConstructCategory::Religion,
            founder,
            1,
        );

        let result = registry.update_property(
            id,
            "deity",
            "Sol",
            5,
            Some(founder),
        );
        assert!(result.is_ok());

        let c = registry.get_construct(id);
        assert!(c.is_some_and(|c| c.properties.get("deity").map(String::as_str) == Some("Sol")));
    }

    #[test]
    fn update_property_on_disbanded_fails() {
        let founder = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Gone",
            SocialConstructCategory::Cultural,
            founder,
            1,
        );
        let _ = registry.disband_construct(id, 5, None);

        let result = registry.update_property(
            id,
            "key",
            "value",
            10,
            None,
        );
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 4. Disband
    // -----------------------------------------------------------------------

    #[test]
    fn disband_construct_clears_members() {
        let founder = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Doomed",
            SocialConstructCategory::Governance,
            founder,
            1,
        );
        let _ = registry.add_member(id, AgentId::new(), 2);
        let _ = registry.add_member(id, AgentId::new(), 3);

        let result = registry.disband_construct(id, 10, None);
        assert!(result.is_ok());

        let c = registry.get_construct(id);
        assert!(c.is_some_and(|c| !c.is_active()));
        assert!(c.is_some_and(|c| c.adherent_count() == 0));
    }

    #[test]
    fn disband_already_disbanded_fails() {
        let founder = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Twice Dead",
            SocialConstructCategory::Religion,
            founder,
            1,
        );
        let r1 = registry.disband_construct(id, 5, None);
        assert!(r1.is_ok());

        let r2 = registry.disband_construct(id, 10, None);
        assert!(r2.is_err());
    }

    // -----------------------------------------------------------------------
    // 5. Queries
    // -----------------------------------------------------------------------

    #[test]
    fn get_by_category_filters() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();

        registry.register_construct(
            String::from("R1"),
            SocialConstructCategory::Religion,
            Some(f),
            1,
            HashMap::new(),
        );
        registry.register_construct(
            String::from("G1"),
            SocialConstructCategory::Governance,
            Some(f),
            2,
            HashMap::new(),
        );
        registry.register_construct(
            String::from("R2"),
            SocialConstructCategory::Religion,
            Some(f),
            3,
            HashMap::new(),
        );

        let religions = registry.get_by_category(SocialConstructCategory::Religion);
        assert_eq!(religions.len(), 2);

        let govs = registry.get_by_category(SocialConstructCategory::Governance);
        assert_eq!(govs.len(), 1);

        let econs = registry.get_by_category(SocialConstructCategory::Economic);
        assert!(econs.is_empty());
    }

    #[test]
    fn get_agent_constructs_returns_memberships() {
        let mut registry = ConstructRegistry::new();
        let agent = AgentId::new();

        let id1 = registry.register_construct(
            String::from("C1"),
            SocialConstructCategory::Cultural,
            Some(agent),
            1,
            HashMap::new(),
        );
        let id2 = registry.register_construct(
            String::from("C2"),
            SocialConstructCategory::Economic,
            None,
            2,
            HashMap::new(),
        );
        let _ = registry.add_member(id2, agent, 3);

        let memberships = registry.get_agent_constructs(agent);
        assert_eq!(memberships.len(), 2);

        let ids: HashSet<Uuid> = memberships.iter().map(|c| c.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn active_count_excludes_disbanded() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();

        let id1 = registry.register_construct(
            String::from("A"),
            SocialConstructCategory::Religion,
            Some(f),
            1,
            HashMap::new(),
        );
        registry.register_construct(
            String::from("B"),
            SocialConstructCategory::Religion,
            Some(f),
            2,
            HashMap::new(),
        );

        assert_eq!(registry.active_count(), 2);

        let _ = registry.disband_construct(id1, 5, None);
        assert_eq!(registry.active_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 6. Merge
    // -----------------------------------------------------------------------

    #[test]
    fn merge_larger_absorbs_smaller() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();

        let id_large = registry.register_construct(
            String::from("Big"),
            SocialConstructCategory::Religion,
            Some(f),
            1,
            HashMap::new(),
        );
        let _ = registry.add_member(id_large, AgentId::new(), 2);
        let _ = registry.add_member(id_large, AgentId::new(), 3);
        // Big has 3 members (founder + 2)

        let id_small = registry.register_construct(
            String::from("Small"),
            SocialConstructCategory::Religion,
            Some(AgentId::new()),
            1,
            HashMap::new(),
        );
        // Small has 1 member (founder)

        let result = registry.merge_constructs(id_large, id_small, 10, Some(f));
        assert!(result.is_ok());

        let survivor_id = result.unwrap_or(Uuid::nil());
        assert_eq!(survivor_id, id_large);

        // Absorbed construct is disbanded
        let absorbed = registry.get_construct(id_small);
        assert!(absorbed.is_some_and(|c| !c.is_active()));

        // Survivor has all members (3 from big + 1 from small, but small's member
        // was added before disband cleared them -- the merge copies first)
        let survivor = registry.get_construct(id_large);
        assert!(survivor.is_some_and(|c| c.is_active()));
    }

    #[test]
    fn merge_same_size_first_absorbs() {
        let mut registry = ConstructRegistry::new();
        let f1 = AgentId::new();
        let f2 = AgentId::new();

        let id_a = registry.register_construct(
            String::from("A"),
            SocialConstructCategory::Governance,
            Some(f1),
            1,
            HashMap::new(),
        );
        let id_b = registry.register_construct(
            String::from("B"),
            SocialConstructCategory::Governance,
            Some(f2),
            1,
            HashMap::new(),
        );
        // Both have 1 member

        let result = registry.merge_constructs(id_a, id_b, 10, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(Uuid::nil()), id_a);
    }

    #[test]
    fn merge_self_fails() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();
        let id = registry.register_construct(
            String::from("X"),
            SocialConstructCategory::Cultural,
            Some(f),
            1,
            HashMap::new(),
        );

        let result = registry.merge_constructs(id, id, 5, None);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 7. Schism
    // -----------------------------------------------------------------------

    #[test]
    fn schism_splits_members() {
        let mut registry = ConstructRegistry::new();
        let founder = AgentId::new();
        let member_a = AgentId::new();
        let member_b = AgentId::new();

        let original_id = registry.register_construct(
            String::from("Original Church"),
            SocialConstructCategory::Religion,
            Some(founder),
            1,
            HashMap::new(),
        );
        let _ = registry.add_member(original_id, member_a, 2);
        let _ = registry.add_member(original_id, member_b, 3);
        // Original has: founder, member_a, member_b

        let mut splinter_set = HashSet::new();
        splinter_set.insert(member_b);

        let result = registry.schism(
            original_id,
            String::from("Reform Church"),
            &splinter_set,
            10,
            Some(member_b),
        );
        assert!(result.is_ok());

        let splinter_id = result.unwrap_or(Uuid::nil());

        // Original still has founder and member_a
        let original = registry.get_construct(original_id);
        assert!(original.is_some_and(|c| c.adherent_count() == 2));
        assert!(original.is_some_and(|c| !c.adherent_ids.contains(&member_b)));

        // Splinter has member_b
        let splinter = registry.get_construct(splinter_id);
        assert!(splinter.is_some_and(|c| c.adherent_count() == 1));
        assert!(splinter.is_some_and(|c| c.adherent_ids.contains(&member_b)));
        assert!(splinter.is_some_and(|c| c.category == SocialConstructCategory::Religion));
    }

    #[test]
    fn schism_empty_splinter_fails() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();
        let id = registry.register_construct(
            String::from("Stable"),
            SocialConstructCategory::Governance,
            Some(f),
            1,
            HashMap::new(),
        );

        let empty = HashSet::new();
        let result = registry.schism(id, String::from("Ghost"), &empty, 5, None);
        assert!(result.is_err());
    }

    #[test]
    fn schism_disbanded_construct_fails() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();
        let id = registry.register_construct(
            String::from("Dead"),
            SocialConstructCategory::Cultural,
            Some(f),
            1,
            HashMap::new(),
        );
        let _ = registry.disband_construct(id, 5, None);

        let mut splinters = HashSet::new();
        splinters.insert(f);

        let result = registry.schism(id, String::from("New"), &splinters, 10, None);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 8. Evolution history tracking
    // -----------------------------------------------------------------------

    #[test]
    fn evolution_history_tracks_all_events() {
        let founder = AgentId::new();
        let member = AgentId::new();
        let (mut registry, id) = make_registry_with_construct(
            "Tracked",
            SocialConstructCategory::Cultural,
            founder,
            1,
        );

        // Founded (1 event)
        let _ = registry.add_member(id, member, 2);       // +1
        let _ = registry.update_property(                  // +1
            id,
            "motto",
            "unity",
            3,
            Some(founder),
        );
        let _ = registry.remove_member(id, member, 4);    // +1

        let c = registry.get_construct(id);
        // 1 (founded) + 1 (join) + 1 (property) + 1 (leave) = 4
        assert!(c.is_some_and(|c| c.evolution_history.len() == 4));
    }

    #[test]
    fn merge_records_events_on_both() {
        let mut registry = ConstructRegistry::new();
        let f = AgentId::new();

        let id_a = registry.register_construct(
            String::from("A"),
            SocialConstructCategory::Religion,
            Some(f),
            1,
            HashMap::new(),
        );
        let id_b = registry.register_construct(
            String::from("B"),
            SocialConstructCategory::Religion,
            Some(AgentId::new()),
            1,
            HashMap::new(),
        );

        let _ = registry.merge_constructs(id_a, id_b, 10, Some(f));

        // Absorber should have Founded + Merged events
        let absorber = registry.get_construct(id_a);
        assert!(absorber.is_some_and(|c| c.evolution_history.len() == 2));

        // Absorbed should have Founded + Disbanded events
        let absorbed = registry.get_construct(id_b);
        assert!(absorbed.is_some_and(|c| c.evolution_history.len() == 2));
    }

    #[test]
    fn schism_records_events_on_both() {
        let mut registry = ConstructRegistry::new();
        let founder = AgentId::new();
        let member = AgentId::new();

        let original_id = registry.register_construct(
            String::from("Origin"),
            SocialConstructCategory::Governance,
            Some(founder),
            1,
            HashMap::new(),
        );
        let _ = registry.add_member(original_id, member, 2);

        let mut splinters = HashSet::new();
        splinters.insert(member);

        let splinter_id = registry
            .schism(original_id, String::from("Splinter"), &splinters, 10, Some(member))
            .unwrap_or(Uuid::nil());

        // Original: Founded + MemberJoined + Schism = 3
        let original = registry.get_construct(original_id);
        assert!(original.is_some_and(|c| c.evolution_history.len() == 3));

        // Splinter: Founded = 1
        let splinter = registry.get_construct(splinter_id);
        assert!(splinter.is_some_and(|c| c.evolution_history.len() == 1));
    }

    // -----------------------------------------------------------------------
    // 9. Initial properties
    // -----------------------------------------------------------------------

    #[test]
    fn register_with_initial_properties() {
        let mut registry = ConstructRegistry::new();
        let mut props = HashMap::new();
        props.insert(String::from("currency"), String::from("shells"));
        props.insert(String::from("tax_rate"), String::from("10%"));

        let id = registry.register_construct(
            String::from("Trade Guild"),
            SocialConstructCategory::Economic,
            Some(AgentId::new()),
            1,
            props,
        );

        let c = registry.get_construct(id);
        assert!(c.is_some_and(|c| c.properties.len() == 2));
        assert!(
            c.is_some_and(|c| c.properties.get("currency").map(String::as_str) == Some("shells"))
        );
    }

    // -----------------------------------------------------------------------
    // 10. Construct not found errors
    // -----------------------------------------------------------------------

    #[test]
    fn get_construct_nonexistent_returns_none() {
        let registry = ConstructRegistry::new();
        assert!(registry.get_construct(Uuid::nil()).is_none());
    }

    #[test]
    fn add_member_nonexistent_construct_fails() {
        let mut registry = ConstructRegistry::new();
        let result = registry.add_member(Uuid::nil(), AgentId::new(), 1);
        assert!(result.is_err());
    }
}
