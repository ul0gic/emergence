//! Knowledge base, discovery mechanics, and tech tree for agents.
//!
//! This module implements the knowledge system from `world-engine.md` section 8:
//!
//! - [`KnowledgeBase`] -- per-agent knowledge storage with discovery tracking
//! - [`DiscoveryMethod`] -- how a concept was learned
//! - [`seed_knowledge`] -- starting knowledge sets by level (0--5)
//! - [`TechTree`] -- prerequisite graph for discoverable concepts
//! - [`attempt_discovery`] -- probabilistic discovery resolution
//!
//! Knowledge determines which actions an agent can perform and influences
//! effectiveness. Knowledge is never lost once acquired.

use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use emergence_types::Personality;

// ---------------------------------------------------------------------------
// DiscoveryMethod
// ---------------------------------------------------------------------------

/// How an agent acquired a piece of knowledge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// Included in the agent's starting knowledge set at creation.
    Seed,
    /// Discovered through repeated use of related concepts.
    Experimentation,
    /// Learned by watching another agent perform an unknown action.
    Observation,
    /// Explicitly taught by another agent via the teach action.
    Taught,
    /// Acquired by reading from a library structure.
    Read,
    /// Small random chance discovery each tick.
    Accidental,
}

// ---------------------------------------------------------------------------
// KnowledgeBase
// ---------------------------------------------------------------------------

/// Per-agent knowledge storage with discovery tracking.
///
/// Tracks which concepts the agent knows, when each was learned,
/// and how it was learned. Knowledge is never lost once acquired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBase {
    /// Set of all known concept identifiers.
    known_concepts: BTreeSet<String>,
    /// Maps each concept to the tick when it was learned.
    discovery_tick: BTreeMap<String, u64>,
    /// Maps each concept to how it was learned.
    discovery_method: BTreeMap<String, DiscoveryMethod>,
}

impl KnowledgeBase {
    /// Create an empty knowledge base.
    pub const fn new() -> Self {
        Self {
            known_concepts: BTreeSet::new(),
            discovery_tick: BTreeMap::new(),
            discovery_method: BTreeMap::new(),
        }
    }

    /// Create a knowledge base pre-populated with seed knowledge at the given level.
    ///
    /// All concepts are recorded as learned at tick 0 via [`DiscoveryMethod::Seed`].
    pub fn with_seed_knowledge(level: u8) -> Self {
        let mut kb = Self::new();
        for concept in seed_knowledge(level) {
            kb.known_concepts.insert(concept.clone());
            kb.discovery_tick.insert(concept.clone(), 0);
            kb.discovery_method
                .insert(concept, DiscoveryMethod::Seed);
        }
        kb
    }

    /// Check whether the agent knows a given concept.
    pub fn knows(&self, concept: &str) -> bool {
        self.known_concepts.contains(concept)
    }

    /// Learn a new concept. If the concept is already known, this is a no-op.
    ///
    /// Records the tick and method of discovery.
    pub fn learn(&mut self, concept: &str, tick: u64, method: DiscoveryMethod) {
        if self.known_concepts.insert(String::from(concept)) {
            self.discovery_tick.insert(String::from(concept), tick);
            self.discovery_method.insert(String::from(concept), method);
        }
    }

    /// Return the number of known concepts.
    pub fn known_count(&self) -> usize {
        self.known_concepts.len()
    }

    /// Return an immutable reference to the set of known concepts.
    pub const fn known_concepts(&self) -> &BTreeSet<String> {
        &self.known_concepts
    }

    /// Return the tick when a concept was learned, if known.
    pub fn discovery_tick_for(&self, concept: &str) -> Option<u64> {
        self.discovery_tick.get(concept).copied()
    }

    /// Return how a concept was learned, if known.
    pub fn discovery_method_for(&self, concept: &str) -> Option<&DiscoveryMethod> {
        self.discovery_method.get(concept)
    }

    /// Sync the knowledge base state to an [`AgentState`] knowledge set.
    ///
    /// This copies the known concepts into the agent state's knowledge field
    /// so it stays consistent with the `KnowledgeBase`.
    pub fn sync_to_agent_state(&self, state: &mut emergence_types::AgentState) {
        state.knowledge.clone_from(&self.known_concepts);
    }
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Seed Knowledge Levels (world-engine.md section 8.2)
// ---------------------------------------------------------------------------

/// Return the concepts added at a specific seed level (not cumulative).
const fn level_concepts(level: u8) -> &'static [&'static str] {
    match level {
        0 => &["exist", "perceive", "move", "basic_communication"],
        1 => &[
            "gather_food", "gather_wood", "gather_stone", "drink_water",
            "eat", "rest", "build_campfire", "build_lean_to", "basic_trade",
        ],
        2 => &[
            "agriculture", "build_hut", "build_storage", "pottery",
            "animal_tracking", "basic_medicine", "barter_system",
            "group_formation", "territorial_claim", "oral_tradition",
        ],
        3 => &[
            "metalworking", "build_forge", "masonry", "written_language",
            "currency_concept", "legislation", "organized_labor",
            "build_wall", "basic_engineering", "bridge_building",
        ],
        4 => &[
            "advanced_tools", "build_workshop", "build_market", "build_library",
            "irrigation", "animal_husbandry", "weaving", "carpentry",
            "stonecutting", "taxation",
        ],
        // Level 5+
        _ => &[
            "advanced_metallurgy", "architecture", "governance", "justice_system",
            "diplomacy", "advanced_agriculture", "medicine", "astronomy",
            "mathematics", "philosophy",
        ],
    }
}

/// Return the set of starting knowledge concepts for a given seed level.
///
/// Levels are cumulative: level 2 includes everything from levels 0 and 1.
///
/// - Level 0: Blank slate -- exist, perceive, move, basic communication
/// - Level 1: Primitive -- survival actions, basic building, trade
/// - Level 2: Ancient -- agriculture, improved building, social structures
/// - Level 3: Medieval -- metalworking, written language, governance
/// - Level 4: Renaissance -- advanced construction, engineering, economy
/// - Level 5: Industrial -- manufacturing, complex governance
///
/// Unknown levels (> 5) return the level 5 set.
pub fn seed_knowledge(level: u8) -> Vec<String> {
    let capped = level.min(5);
    let mut all = Vec::new();
    for l in 0..=capped {
        for concept in level_concepts(l) {
            all.push(String::from(*concept));
        }
    }
    all
}

// ---------------------------------------------------------------------------
// TechTree (world-engine.md section 8.4)
// ---------------------------------------------------------------------------

/// The discovery adjacency map -- a prerequisite graph for knowledge concepts.
///
/// Each entry maps a discoverable concept to the set of concepts required
/// before it can be discovered. Agents cannot discover a concept unless they
/// already know all prerequisites.
#[derive(Debug, Clone)]
pub struct TechTree {
    /// Maps each concept to its prerequisite set.
    prerequisites: BTreeMap<String, BTreeSet<String>>,
}

impl TechTree {
    /// Create the canonical tech tree from `world-engine.md` section 8.4.
    pub fn new() -> Self {
        let mut prerequisites: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        // Helper to insert prerequisite entries.
        let mut add = |concept: &str, prereqs: &[&str]| {
            prerequisites.insert(
                String::from(concept),
                prereqs.iter().map(|s| String::from(*s)).collect(),
            );
        };

        // --- Tier 1 discoveries (from Tier 0 knowledge) ---
        add("cooking", &["gather_food", "build_campfire"]);
        add("basic_tools", &["gather_wood", "gather_stone"]);
        add("observe_seasons", &["perceive", "gather_food"]);
        add("fishing", &["perceive", "gather_food"]);
        add("hunting", &["basic_tools", "animal_tracking"]);

        // --- Tier 2 discoveries ---
        add("agriculture", &["gather_food", "observe_seasons"]);
        add("food_preservation", &["agriculture", "build_storage"]);
        add("mining", &["basic_tools", "gather_stone"]);
        add("smelting", &["mining", "build_campfire"]);
        add("pottery", &["gather_stone", "build_campfire"]);
        add("animal_tracking", &["perceive", "move"]);
        add("basic_medicine", &["gather_food", "observe_seasons"]);
        add("build_hut", &["build_lean_to", "gather_stone"]);
        add("build_storage", &["build_lean_to", "gather_stone"]);
        add("oral_tradition", &["basic_communication", "group_formation"]);
        add("barter_system", &["basic_trade", "group_formation"]);
        add("group_formation", &["basic_communication", "basic_trade"]);
        add("territorial_claim", &["group_formation", "build_hut"]);

        // --- Tier 3 discoveries ---
        add("metalworking", &["smelting", "basic_tools"]);
        add("build_forge", &["smelting", "masonry"]);
        add("masonry", &["build_hut", "gather_stone"]);
        add("written_language", &["oral_tradition", "pottery"]);
        add("currency_concept", &["barter_system", "written_language"]);
        add("legislation", &["governance", "written_language"]);
        add("organized_labor", &["group_formation", "territorial_claim"]);
        add("build_wall", &["masonry", "territorial_claim"]);
        add("basic_engineering", &["masonry", "basic_tools"]);
        add("bridge_building", &["basic_engineering", "gather_wood"]);
        add("governance", &["group_formation", "territorial_claim"]);

        // --- Tier 4 discoveries ---
        add("advanced_tools", &["metalworking", "basic_tools"]);
        add("build_workshop", &["basic_tools", "build_hut"]);
        add("build_market", &["barter_system", "build_hut"]);
        add("build_library", &["written_language", "build_hut"]);
        add("irrigation", &["agriculture", "basic_engineering"]);
        add("animal_husbandry", &["hunting", "agriculture"]);
        add("weaving", &["gather_food", "basic_tools"]);
        add("carpentry", &["basic_tools", "gather_wood"]);
        add("stonecutting", &["basic_tools", "gather_stone"]);
        add("taxation", &["currency_concept", "governance"]);

        // --- Tier 5 discoveries ---
        add("advanced_metallurgy", &["metalworking", "build_forge"]);
        add("architecture", &["masonry", "basic_engineering"]);
        add("justice_system", &["legislation", "governance"]);
        add("diplomacy", &["governance", "oral_tradition"]);
        add("advanced_agriculture", &["agriculture", "irrigation"]);
        add("medicine", &["basic_medicine", "written_language"]);
        add("astronomy", &["observe_seasons", "mathematics"]);
        add("mathematics", &["written_language", "basic_tools"]);
        add("philosophy", &["oral_tradition", "written_language"]);

        Self { prerequisites }
    }

    /// Check whether an agent can discover a specific concept given their
    /// current knowledge.
    ///
    /// Returns `true` if:
    /// - The concept exists in the tech tree
    /// - The agent knows all prerequisites
    /// - The agent does NOT already know this concept
    pub fn can_discover(&self, concept: &str, known: &BTreeSet<String>) -> bool {
        // Already known -- cannot "discover" it again
        if known.contains(concept) {
            return false;
        }

        // Look up prerequisites -- concept must exist and all prereqs must be met
        self.prerequisites
            .get(concept)
            .is_some_and(|prereqs| prereqs.iter().all(|p| known.contains(p)))
    }

    /// Return all concepts the agent could potentially discover given what
    /// they currently know.
    ///
    /// A concept is "available" if the agent knows all its prerequisites
    /// but does not yet know the concept itself.
    pub fn available_discoveries(&self, known: &BTreeSet<String>) -> Vec<String> {
        self.prerequisites
            .iter()
            .filter_map(|(concept, prereqs)| {
                if known.contains(concept) {
                    return None;
                }
                if prereqs.iter().all(|p| known.contains(p)) {
                    Some(concept.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Return the prerequisites for a concept, if it exists in the tree.
    pub fn prerequisites_for(&self, concept: &str) -> Option<&BTreeSet<String>> {
        self.prerequisites.get(concept)
    }
}

impl Default for TechTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Discovery Mechanics (world-engine.md section 8.3)
// ---------------------------------------------------------------------------

/// Configuration for discovery probability rates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    /// Base chance of experimentation discovery per tick (default: 2%, stored
    /// as numerator over 10000 for integer arithmetic: 200 = 2%).
    pub experimentation_chance_per_10000: u32,
    /// Base teaching success rate as a percentage (default: 80).
    pub teaching_success_base_pct: u32,
    /// Per-skill-level bonus to teaching success as a percentage (default: 5).
    pub teaching_skill_bonus_pct: u32,
    /// Maximum teaching success rate as a percentage (default: 99).
    pub teaching_max_success_pct: u32,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            experimentation_chance_per_10000: 200,
            teaching_success_base_pct: 80,
            teaching_skill_bonus_pct: 5,
            teaching_max_success_pct: 99,
        }
    }
}

/// Attempt a discovery for an agent based on the given method.
///
/// For [`DiscoveryMethod::Experimentation`]: picks a random available
/// discovery from the tech tree and rolls against the experimentation chance.
///
/// For [`DiscoveryMethod::Observation`]: chance equals
/// `curiosity * 0.3` (as a percentage of 10000). Picks a random available
/// discovery.
///
/// For [`DiscoveryMethod::Accidental`]: small random chance each tick,
/// weighted by curiosity. Picks a random available discovery.
///
/// Returns `Some(concept)` if the agent discovers something, or `None` if
/// the roll fails or no discoveries are available.
pub fn attempt_discovery(
    agent_knowledge: &KnowledgeBase,
    tech_tree: &TechTree,
    personality: &Personality,
    method: &DiscoveryMethod,
    config: &DiscoveryConfig,
    rng: &mut impl Rng,
) -> Option<String> {
    let available = tech_tree.available_discoveries(agent_knowledge.known_concepts());
    if available.is_empty() {
        return None;
    }

    let chance_per_10000: u32 = match method {
        DiscoveryMethod::Experimentation => config.experimentation_chance_per_10000,
        DiscoveryMethod::Observation => {
            // curiosity * 0.3, expressed per 10000
            // curiosity is 0.0--1.0 as Decimal. Convert to u32 in range 0--10000.
            let curiosity_pct = decimal_to_per_10000(personality.curiosity);
            // Multiply by 3000 / 10000 = 0.3
            curiosity_pct.saturating_mul(3000).checked_div(10000).unwrap_or(0)
        }
        DiscoveryMethod::Accidental => {
            // Half the experimentation chance, weighted by curiosity
            let base = config
                .experimentation_chance_per_10000
                .checked_div(2)
                .unwrap_or(0);
            let curiosity_pct = decimal_to_per_10000(personality.curiosity);
            base.saturating_mul(curiosity_pct)
                .checked_div(10000)
                .unwrap_or(0)
        }
        // Seed, Taught, Read are not random -- they are deterministic
        DiscoveryMethod::Seed | DiscoveryMethod::Taught | DiscoveryMethod::Read => return None,
    };

    // Roll the dice (0..10000)
    let roll: u32 = rng.random_range(0..10000);

    if roll < chance_per_10000 {
        // Pick a random concept from available discoveries
        let idx: usize = rng.random_range(0..available.len());
        available.into_iter().nth(idx)
    } else {
        None
    }
}

/// Convert a [`Decimal`] in range 0.0--1.0 to an integer in range 0--10000.
///
/// Clamps to bounds. Returns 0 on conversion failure.
fn decimal_to_per_10000(d: Decimal) -> u32 {
    // Clamp to 0.0..=1.0 first
    let clamped = if d < Decimal::ZERO {
        Decimal::ZERO
    } else if d > Decimal::ONE {
        Decimal::ONE
    } else {
        d
    };
    // Multiply by 10000 using checked arithmetic, then truncate to integer.
    let ten_k = Decimal::from(10000);
    let scaled = clamped.checked_mul(ten_k).unwrap_or(ten_k).trunc();
    // Convert to u32 via mantissa and scale.  After `trunc()`, the value is
    // mathematically an integer but may still carry a non-zero scale
    // (e.g. mantissa 50000 with scale 1 representing 5000).  We divide out the
    // scale factor to obtain the true integer value.
    let mantissa = scaled.mantissa();
    let scale = scaled.scale();
    // Compute 10^scale as i128 to divide out fractional digits.
    let divisor: i128 = 10_i128.checked_pow(scale).unwrap_or(1);
    let val = mantissa.checked_div(divisor).unwrap_or(0);
    if val < 0 {
        0
    } else if val > 10000 {
        10000
    } else {
        // Safe: we verified 0 <= val <= 10000 which fits in u32.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let result = val as u32;
        result
    }
}

/// Evaluate whether a teach action succeeds.
///
/// Success rate: `base_pct + (teacher_skill_level * bonus_pct)`, capped at
/// `max_pct`. Returns `true` if the roll succeeds.
pub fn attempt_teach(
    teacher_skill_level: u32,
    config: &DiscoveryConfig,
    rng: &mut impl Rng,
) -> bool {
    let bonus = teacher_skill_level
        .saturating_mul(config.teaching_skill_bonus_pct);
    let total_pct = config
        .teaching_success_base_pct
        .saturating_add(bonus)
        .min(config.teaching_max_success_pct);

    let roll: u32 = rng.random_range(0..100);
    roll < total_pct
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use rust_decimal::Decimal;

    use super::*;

    fn test_personality() -> Personality {
        Personality {
            curiosity: Decimal::new(5, 1), // 0.5
            cooperation: Decimal::new(5, 1),
            aggression: Decimal::new(3, 1),
            risk_tolerance: Decimal::new(4, 1),
            industriousness: Decimal::new(6, 1),
            sociability: Decimal::new(7, 1),
            honesty: Decimal::new(8, 1),
            loyalty: Decimal::new(5, 1),
        }
    }

    // -----------------------------------------------------------------------
    // KnowledgeBase
    // -----------------------------------------------------------------------

    #[test]
    fn empty_knowledge_base() {
        let kb = KnowledgeBase::new();
        assert_eq!(kb.known_count(), 0);
        assert!(!kb.knows("gather_food"));
    }

    #[test]
    fn learn_adds_concept() {
        let mut kb = KnowledgeBase::new();
        kb.learn("gather_food", 10, DiscoveryMethod::Experimentation);
        assert!(kb.knows("gather_food"));
        assert_eq!(kb.known_count(), 1);
        assert_eq!(kb.discovery_tick_for("gather_food"), Some(10));
        assert_eq!(
            kb.discovery_method_for("gather_food"),
            Some(&DiscoveryMethod::Experimentation)
        );
    }

    #[test]
    fn learn_duplicate_is_no_op() {
        let mut kb = KnowledgeBase::new();
        kb.learn("gather_food", 10, DiscoveryMethod::Experimentation);
        kb.learn("gather_food", 20, DiscoveryMethod::Taught);
        // Should keep the original tick and method
        assert_eq!(kb.known_count(), 1);
        assert_eq!(kb.discovery_tick_for("gather_food"), Some(10));
        assert_eq!(
            kb.discovery_method_for("gather_food"),
            Some(&DiscoveryMethod::Experimentation)
        );
    }

    #[test]
    fn with_seed_knowledge_level_0() {
        let kb = KnowledgeBase::with_seed_knowledge(0);
        assert_eq!(kb.known_count(), 4);
        assert!(kb.knows("exist"));
        assert!(kb.knows("perceive"));
        assert!(kb.knows("move"));
        assert!(kb.knows("basic_communication"));
        assert!(!kb.knows("gather_food"));
    }

    #[test]
    fn with_seed_knowledge_level_1() {
        let kb = KnowledgeBase::with_seed_knowledge(1);
        assert!(kb.known_count() > 4);
        assert!(kb.knows("exist"));
        assert!(kb.knows("gather_food"));
        assert!(kb.knows("build_campfire"));
        assert!(kb.knows("basic_trade"));
        assert!(!kb.knows("agriculture"));
    }

    #[test]
    fn sync_to_agent_state() {
        let mut kb = KnowledgeBase::new();
        kb.learn("gather_food", 1, DiscoveryMethod::Seed);
        kb.learn("build_campfire", 1, DiscoveryMethod::Seed);

        let mut state = emergence_types::AgentState {
            agent_id: emergence_types::AgentId::new(),
            energy: 80,
            health: 100,
            hunger: 0,
            age: 0,
            born_at_tick: 0,
            location_id: emergence_types::LocationId::new(),
            destination_id: None,
            travel_progress: 0,
            inventory: std::collections::BTreeMap::new(),
            carry_capacity: 50,
            knowledge: BTreeSet::new(),
            skills: std::collections::BTreeMap::new(),
            skill_xp: std::collections::BTreeMap::new(),
            goals: Vec::new(),
            relationships: std::collections::BTreeMap::new(),
            memory: Vec::new(),
        };

        kb.sync_to_agent_state(&mut state);
        assert!(state.knowledge.contains("gather_food"));
        assert!(state.knowledge.contains("build_campfire"));
        assert_eq!(state.knowledge.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Seed Knowledge Levels
    // -----------------------------------------------------------------------

    #[test]
    fn seed_knowledge_level_0_count() {
        let k = seed_knowledge(0);
        assert_eq!(k.len(), 4);
    }

    #[test]
    fn seed_knowledge_level_1_count() {
        let k = seed_knowledge(1);
        assert_eq!(k.len(), 13); // 4 + 9
    }

    #[test]
    fn seed_knowledge_level_2_count() {
        let k = seed_knowledge(2);
        assert_eq!(k.len(), 23); // 13 + 10
    }

    #[test]
    fn seed_knowledge_level_3_count() {
        let k = seed_knowledge(3);
        assert_eq!(k.len(), 33); // 23 + 10
    }

    #[test]
    fn seed_knowledge_level_4_count() {
        let k = seed_knowledge(4);
        assert_eq!(k.len(), 43); // 33 + 10
    }

    #[test]
    fn seed_knowledge_level_5_count() {
        let k = seed_knowledge(5);
        assert_eq!(k.len(), 53); // 43 + 10
    }

    #[test]
    fn seed_knowledge_level_is_cumulative() {
        let k1: BTreeSet<String> = seed_knowledge(1).into_iter().collect();
        let k2: BTreeSet<String> = seed_knowledge(2).into_iter().collect();
        // Level 2 is a superset of level 1
        assert!(k1.is_subset(&k2));
    }

    #[test]
    fn seed_knowledge_above_5_returns_level_5() {
        let k5 = seed_knowledge(5);
        let k99 = seed_knowledge(99);
        assert_eq!(k5, k99);
    }

    // -----------------------------------------------------------------------
    // TechTree
    // -----------------------------------------------------------------------

    #[test]
    fn tech_tree_can_discover_with_prereqs_met() {
        let tree = TechTree::new();
        let mut known: BTreeSet<String> = BTreeSet::new();
        known.insert(String::from("gather_food"));
        known.insert(String::from("build_campfire"));

        assert!(tree.can_discover("cooking", &known));
    }

    #[test]
    fn tech_tree_cannot_discover_missing_prereqs() {
        let tree = TechTree::new();
        let mut known: BTreeSet<String> = BTreeSet::new();
        known.insert(String::from("gather_food"));
        // Missing build_campfire

        assert!(!tree.can_discover("cooking", &known));
    }

    #[test]
    fn tech_tree_cannot_discover_already_known() {
        let tree = TechTree::new();
        let mut known: BTreeSet<String> = BTreeSet::new();
        known.insert(String::from("gather_food"));
        known.insert(String::from("build_campfire"));
        known.insert(String::from("cooking"));

        assert!(!tree.can_discover("cooking", &known));
    }

    #[test]
    fn tech_tree_unknown_concept() {
        let tree = TechTree::new();
        let known: BTreeSet<String> = BTreeSet::new();
        assert!(!tree.can_discover("quantum_physics", &known));
    }

    #[test]
    fn tech_tree_available_discoveries_empty_knowledge() {
        let tree = TechTree::new();
        let known: BTreeSet<String> = BTreeSet::new();
        // With no knowledge, nothing should be discoverable (all concepts have prereqs)
        let available = tree.available_discoveries(&known);
        assert!(available.is_empty());
    }

    #[test]
    fn tech_tree_available_discoveries_level_1() {
        let tree = TechTree::new();
        let known: BTreeSet<String> = seed_knowledge(1).into_iter().collect();

        let available = tree.available_discoveries(&known);
        // With level 1 knowledge, several things become discoverable
        assert!(!available.is_empty());
        // cooking requires gather_food + build_campfire -- both known at level 1
        assert!(available.contains(&String::from("cooking")));
        // basic_tools requires gather_wood + gather_stone -- both known at level 1
        assert!(available.contains(&String::from("basic_tools")));
    }

    #[test]
    fn tech_tree_prerequisites_for_cooking() {
        let tree = TechTree::new();
        let prereqs = tree.prerequisites_for("cooking");
        assert!(prereqs.is_some());
        let empty = BTreeSet::new();
        let prereqs = prereqs.unwrap_or(&empty);
        assert!(prereqs.contains("gather_food"));
        assert!(prereqs.contains("build_campfire"));
        assert_eq!(prereqs.len(), 2);
    }

    #[test]
    fn tech_tree_chain_discovery() {
        let tree = TechTree::new();
        let mut known: BTreeSet<String> = seed_knowledge(1).into_iter().collect();

        // Discover basic_tools (prereqs: gather_wood, gather_stone)
        assert!(tree.can_discover("basic_tools", &known));
        known.insert(String::from("basic_tools"));

        // Now mining requires basic_tools + gather_stone
        assert!(tree.can_discover("mining", &known));
        known.insert(String::from("mining"));

        // smelting requires mining + build_campfire
        assert!(tree.can_discover("smelting", &known));
        known.insert(String::from("smelting"));

        // metalworking requires smelting + basic_tools
        assert!(tree.can_discover("metalworking", &known));
    }

    // -----------------------------------------------------------------------
    // Discovery Mechanics
    // -----------------------------------------------------------------------

    #[test]
    fn attempt_discovery_no_available() {
        let kb = KnowledgeBase::new(); // Empty -- nothing discoverable
        let tree = TechTree::new();
        let config = DiscoveryConfig::default();
        let personality = test_personality();
        let mut rng = SmallRng::seed_from_u64(42);

        let result = attempt_discovery(
            &kb,
            &tree,
            &personality,
            &DiscoveryMethod::Experimentation,
            &config,
            &mut rng,
        );
        assert!(result.is_none());
    }

    #[test]
    fn attempt_discovery_experimentation_can_succeed() {
        let kb = KnowledgeBase::with_seed_knowledge(1);
        let tree = TechTree::new();
        // Set chance to 100% for deterministic test
        let config = DiscoveryConfig {
            experimentation_chance_per_10000: 10000,
            ..DiscoveryConfig::default()
        };
        let personality = test_personality();
        let mut rng = SmallRng::seed_from_u64(42);

        let result = attempt_discovery(
            &kb,
            &tree,
            &personality,
            &DiscoveryMethod::Experimentation,
            &config,
            &mut rng,
        );
        assert!(result.is_some());
        // Should be one of the available discoveries
        let available: BTreeSet<String> =
            tree.available_discoveries(kb.known_concepts()).into_iter().collect();
        assert!(available.contains(result.as_deref().unwrap_or("")));
    }

    #[test]
    fn attempt_discovery_zero_chance_never_succeeds() {
        let kb = KnowledgeBase::with_seed_knowledge(1);
        let tree = TechTree::new();
        let config = DiscoveryConfig {
            experimentation_chance_per_10000: 0,
            ..DiscoveryConfig::default()
        };
        let personality = test_personality();
        let mut rng = SmallRng::seed_from_u64(42);

        for _ in 0..100 {
            let result = attempt_discovery(
                &kb,
                &tree,
                &personality,
                &DiscoveryMethod::Experimentation,
                &config,
                &mut rng,
            );
            assert!(result.is_none());
        }
    }

    #[test]
    fn attempt_discovery_seed_method_returns_none() {
        let kb = KnowledgeBase::with_seed_knowledge(1);
        let tree = TechTree::new();
        let config = DiscoveryConfig::default();
        let personality = test_personality();
        let mut rng = SmallRng::seed_from_u64(42);

        let result = attempt_discovery(
            &kb,
            &tree,
            &personality,
            &DiscoveryMethod::Seed,
            &config,
            &mut rng,
        );
        assert!(result.is_none());
    }

    #[test]
    fn attempt_discovery_observation_high_curiosity() {
        let kb = KnowledgeBase::with_seed_knowledge(1);
        let tree = TechTree::new();
        // Use 100% chance by setting observation to always succeed
        let config = DiscoveryConfig::default();
        let mut personality = test_personality();
        personality.curiosity = Decimal::new(10, 1); // 1.0 -- max curiosity
        let mut rng = SmallRng::seed_from_u64(42);

        // With curiosity 1.0, observation chance = 1.0 * 0.3 = 3000/10000 = 30%
        // Over many tries, should succeed at least once
        let mut found = false;
        for _ in 0..100 {
            let result = attempt_discovery(
                &kb,
                &tree,
                &personality,
                &DiscoveryMethod::Observation,
                &config,
                &mut rng,
            );
            if result.is_some() {
                found = true;
                break;
            }
        }
        assert!(found, "Observation should succeed at least once with max curiosity over 100 attempts");
    }

    // -----------------------------------------------------------------------
    // Teaching
    // -----------------------------------------------------------------------

    #[test]
    fn attempt_teach_base_rate() {
        let config = DiscoveryConfig::default();
        let mut rng = SmallRng::seed_from_u64(42);

        // With skill level 0: 80% success rate
        let mut successes: u32 = 0;
        for _ in 0..1000 {
            if attempt_teach(0, &config, &mut rng) {
                successes = successes.saturating_add(1);
            }
        }
        // Should be roughly 80% (between 700 and 900 is reasonable)
        assert!(successes > 700, "Expected >700 successes out of 1000, got {successes}");
        assert!(successes < 900, "Expected <900 successes out of 1000, got {successes}");
    }

    #[test]
    fn attempt_teach_with_skill_bonus() {
        let config = DiscoveryConfig::default();
        let mut rng = SmallRng::seed_from_u64(42);

        // With skill level 3: 80 + 15 = 95% success rate
        let mut successes: u32 = 0;
        for _ in 0..1000 {
            if attempt_teach(3, &config, &mut rng) {
                successes = successes.saturating_add(1);
            }
        }
        // Should be roughly 95%
        assert!(successes > 900, "Expected >900 successes out of 1000, got {successes}");
    }

    #[test]
    fn attempt_teach_capped_at_max() {
        let config = DiscoveryConfig::default();
        let mut rng = SmallRng::seed_from_u64(42);

        // With skill level 100: 80 + 500 = 580, capped at 99%
        let mut successes: u32 = 0;
        for _ in 0..1000 {
            if attempt_teach(100, &config, &mut rng) {
                successes = successes.saturating_add(1);
            }
        }
        // Should be roughly 99%
        assert!(successes > 960, "Expected >960 successes out of 1000, got {successes}");
        // Should not be 100% (cap is 99, so 1% chance of failure)
        // With 1000 trials, having all 1000 succeed is extremely unlikely (p < 0.0001)
    }

    // -----------------------------------------------------------------------
    // Helper: decimal_to_per_10000
    // -----------------------------------------------------------------------

    #[test]
    fn decimal_conversion_midpoint() {
        let result = decimal_to_per_10000(Decimal::new(5, 1)); // 0.5
        assert_eq!(result, 5000);
    }

    #[test]
    fn decimal_conversion_zero() {
        let result = decimal_to_per_10000(Decimal::ZERO);
        assert_eq!(result, 0);
    }

    #[test]
    fn decimal_conversion_one() {
        let result = decimal_to_per_10000(Decimal::ONE);
        assert_eq!(result, 10000);
    }

    #[test]
    fn decimal_conversion_clamped_above() {
        let result = decimal_to_per_10000(Decimal::new(15, 1)); // 1.5
        assert_eq!(result, 10000);
    }
}
