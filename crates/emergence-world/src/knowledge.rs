//! Knowledge tree and tech progression for the Emergence simulation.
//!
//! Implements `world-engine.md` section 8: the discovery adjacency map,
//! prerequisite chains, and era-tagged knowledge items from Primitive
//! through Early Industrial.
//!
//! The knowledge tree is a directed acyclic graph (DAG) where each
//! [`KnowledgeItem`] has zero or more prerequisite IDs. An agent can
//! discover an item only when all prerequisites are present in their
//! knowledge set.
//!
//! # Design
//!
//! - Items are identified by `snake_case` string IDs (e.g. `"crop_rotation"`).
//! - The tree is built once at startup via [`build_extended_tech_tree`] and
//!   shared immutably for the lifetime of the simulation.
//! - [`KnowledgeTree`] provides lookup, prerequisite validation, discovery
//!   candidate computation, and era filtering.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The technological era a knowledge item belongs to.
///
/// Eras are ordered chronologically; items in later eras generally
/// require prerequisites from earlier eras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum KnowledgeEra {
    /// Starting era -- basic survival.
    Primitive,
    /// Early settled communities, agriculture, basic construction.
    BronzeAge,
    /// Advanced metalworking, organized society.
    IronAge,
    /// Written language, governance, philosophy.
    Classical,
    /// Complex institutions, advanced engineering.
    Medieval,
    /// Manufacturing, early mechanization.
    EarlyIndustrial,
}

/// A single item in the knowledge tree.
///
/// Each item represents a concept, technique, or discovery that an
/// agent can acquire. Items form a DAG through their `prerequisites`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeItem {
    /// Unique identifier (`snake_case`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Which era this item belongs to.
    pub era: KnowledgeEra,
    /// IDs of items that must be known before this can be discovered.
    pub prerequisites: Vec<String>,
    /// Narrative description of the item.
    pub description: String,
    /// What action or capability this knowledge unlocks, if any.
    pub unlocks: Option<String>,
}

/// A complete knowledge tree containing all discoverable items.
///
/// Provides efficient lookup by ID, prerequisite validation, and
/// discovery candidate computation.
#[derive(Debug, Clone)]
pub struct KnowledgeTree {
    items: BTreeMap<String, KnowledgeItem>,
}

// ---------------------------------------------------------------------------
// KnowledgeTree implementation
// ---------------------------------------------------------------------------

impl KnowledgeTree {
    /// Create a new knowledge tree from a list of items.
    ///
    /// Duplicates are silently overwritten (last wins).
    pub fn new(items: Vec<KnowledgeItem>) -> Self {
        let mut map = BTreeMap::new();
        for item in items {
            map.insert(item.id.clone(), item);
        }
        Self { items: map }
    }

    /// Look up a knowledge item by ID.
    pub fn get(&self, id: &str) -> Option<&KnowledgeItem> {
        self.items.get(id)
    }

    /// Return the total number of items in the tree.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return all item IDs.
    pub fn all_ids(&self) -> BTreeSet<String> {
        self.items.keys().cloned().collect()
    }

    /// Return all items in a given era.
    pub fn items_in_era(&self, era: KnowledgeEra) -> Vec<&KnowledgeItem> {
        self.items.values().filter(|item| item.era == era).collect()
    }

    /// Check whether all prerequisites for a given item exist in the tree.
    pub fn prerequisites_exist(&self, id: &str) -> bool {
        self.items.get(id).is_some_and(|item| {
            item.prerequisites.iter().all(|prereq| self.items.contains_key(prereq.as_str()))
        })
    }

    /// Check whether an agent with the given knowledge set can discover
    /// a specific item (all prerequisites are met).
    pub fn can_discover(&self, id: &str, agent_knowledge: &BTreeSet<String>) -> bool {
        self.items.get(id).is_some_and(|item| {
            item.prerequisites.iter().all(|prereq| agent_knowledge.contains(prereq.as_str()))
        })
    }

    /// Return all items that an agent could potentially discover given
    /// their current knowledge set.
    ///
    /// An item is a candidate if:
    /// 1. The agent does not already know it.
    /// 2. All prerequisites are satisfied.
    pub fn discovery_candidates(&self, agent_knowledge: &BTreeSet<String>) -> Vec<&KnowledgeItem> {
        self.items
            .values()
            .filter(|item| {
                !agent_knowledge.contains(item.id.as_str())
                    && item
                        .prerequisites
                        .iter()
                        .all(|prereq| agent_knowledge.contains(prereq.as_str()))
            })
            .collect()
    }

    /// Validate that the tree forms a DAG (no cycles) and all
    /// prerequisites reference existing items.
    ///
    /// Returns a list of error messages. An empty list means the tree
    /// is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Check all prerequisites exist.
        for item in self.items.values() {
            for prereq in &item.prerequisites {
                if !self.items.contains_key(prereq.as_str()) {
                    errors.push(format!(
                        "Item '{}' has prerequisite '{}' which does not exist in the tree",
                        item.id, prereq
                    ));
                }
            }
        }

        // Check no self-references.
        for item in self.items.values() {
            if item.prerequisites.contains(&item.id) {
                errors.push(format!("Item '{}' lists itself as a prerequisite", item.id));
            }
        }

        // BFS cycle detection using Kahn's algorithm (topological sort).
        let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
        let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

        for item in self.items.values() {
            in_degree.entry(item.id.as_str()).or_insert(0);
            adjacency.entry(item.id.as_str()).or_default();
            for prereq in &item.prerequisites {
                if self.items.contains_key(prereq.as_str()) {
                    adjacency.entry(prereq.as_str()).or_default().push(item.id.as_str());
                    let entry = in_degree.entry(item.id.as_str()).or_insert(0);
                    *entry = entry.saturating_add(1);
                }
            }
        }

        let mut queue: VecDeque<&str> = VecDeque::new();
        for (&id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(id);
            }
        }

        let mut visited_count: usize = 0;
        while let Some(node) = queue.pop_front() {
            visited_count = visited_count.saturating_add(1);
            if let Some(neighbors) = adjacency.get(node) {
                for &neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        if visited_count != self.items.len() {
            errors.push(String::from(
                "Cycle detected in the knowledge tree -- topological sort could not visit all nodes",
            ));
        }

        errors
    }

    /// Insert a new knowledge item into the tree.
    ///
    /// Returns `true` if the item was newly inserted, `false` if an
    /// item with the same ID already existed (and was replaced).
    pub fn insert(&mut self, item: KnowledgeItem) -> bool {
        self.items.insert(item.id.clone(), item).is_none()
    }

    /// Check whether the tree contains an item with the given ID.
    pub fn contains(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    /// Return all items as a slice-like iterator.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &KnowledgeItem)> {
        self.items.iter()
    }
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

/// Create a knowledge item with all fields specified.
fn item(
    id: &str,
    name: &str,
    era: KnowledgeEra,
    prerequisites: &[&str],
    description: &str,
    unlocks: Option<&str>,
) -> KnowledgeItem {
    KnowledgeItem {
        id: String::from(id),
        name: String::from(name),
        era,
        prerequisites: prerequisites.iter().map(|s| String::from(*s)).collect(),
        description: String::from(description),
        unlocks: unlocks.map(String::from),
    }
}

// ---------------------------------------------------------------------------
// Seed knowledge (from world-engine.md section 8.2)
// ---------------------------------------------------------------------------

/// Build the seed/primitive knowledge items from the spec.
fn seed_items() -> Vec<KnowledgeItem> {
    vec![
        // Level 0 -- Blank Slate
        item("exist", "Existence", KnowledgeEra::Primitive, &[], "Awareness of self and surroundings.", None),
        item("perceive", "Perception", KnowledgeEra::Primitive, &["exist"], "Ability to observe the world.", None),
        item("move", "Movement", KnowledgeEra::Primitive, &["perceive"], "Ability to travel between locations.", Some("move")),
        item("basic_communication", "Basic Communication", KnowledgeEra::Primitive, &["perceive"], "Ability to send simple messages to other agents.", Some("communicate")),

        // Level 1 -- Primitive
        item("gather_food", "Food Gathering", KnowledgeEra::Primitive, &["perceive"], "Ability to gather food from the environment.", Some("gather (food)")),
        item("gather_wood", "Wood Gathering", KnowledgeEra::Primitive, &["perceive"], "Ability to harvest wood from forests.", Some("gather (wood)")),
        item("gather_stone", "Stone Gathering", KnowledgeEra::Primitive, &["perceive"], "Ability to collect stone from rocky areas.", Some("gather (stone)")),
        item("drink_water", "Drinking", KnowledgeEra::Primitive, &["perceive"], "Ability to drink water.", Some("drink")),
        item("eat", "Eating", KnowledgeEra::Primitive, &["perceive"], "Ability to consume food.", Some("eat")),
        item("rest", "Resting", KnowledgeEra::Primitive, &["exist"], "Ability to rest and recover energy.", Some("rest")),
        item("build_campfire", "Campfire Building", KnowledgeEra::Primitive, &["gather_wood"], "Ability to construct a campfire for warmth and cooking.", Some("build campfire")),
        item("build_lean_to", "Lean-to Building", KnowledgeEra::Primitive, &["gather_wood"], "Ability to build a simple lean-to shelter.", Some("build lean-to")),
        item("basic_trade", "Basic Trade", KnowledgeEra::Primitive, &["basic_communication"], "Ability to exchange resources with other agents.", Some("trade_offer")),

        // Level 2 -- Ancient / Bronze Age foundations
        item("observe_seasons", "Seasonal Observation", KnowledgeEra::Primitive, &["perceive"], "Recognition of seasonal patterns in the environment.", None),
        item("animal_tracking", "Animal Tracking", KnowledgeEra::Primitive, &["perceive", "gather_food"], "Ability to track and hunt animals.", Some("gather (meat)")),
        item("cooking", "Cooking", KnowledgeEra::Primitive, &["gather_food", "build_campfire"], "Ability to cook food for improved nutrition.", Some("craft (cooked food)")),
        item("fire_mastery", "Fire Mastery", KnowledgeEra::Primitive, &["build_campfire"], "Advanced understanding of fire and its uses.", None),
    ]
}

/// Build the Bronze Age knowledge items.
fn bronze_age_items() -> Vec<KnowledgeItem> {
    vec![
        item("agriculture", "Agriculture", KnowledgeEra::BronzeAge, &["gather_food", "observe_seasons"], "Knowledge of planting and harvesting crops.", Some("farm_plant, farm_harvest")),
        item("build_hut", "Hut Building", KnowledgeEra::BronzeAge, &["build_lean_to", "gather_stone"], "Ability to construct a sturdy hut with weather protection.", Some("build basic_hut")),
        item("build_storage", "Storage Construction", KnowledgeEra::BronzeAge, &["gather_stone", "build_lean_to"], "Ability to build underground storage pits.", Some("build storage_pit")),
        item("pottery", "Pottery", KnowledgeEra::BronzeAge, &["fire_mastery", "gather_stone"], "Ability to shape and fire clay into useful vessels.", Some("craft (pottery)")),
        item("basic_medicine", "Basic Medicine", KnowledgeEra::BronzeAge, &["gather_food", "animal_tracking"], "Knowledge of healing herbs and basic remedies.", Some("craft (medicine)")),
        item("barter_system", "Barter System", KnowledgeEra::BronzeAge, &["basic_trade", "group_formation"], "Organized exchange of goods between agents.", Some("trade (improved)")),
        item("group_formation", "Group Formation", KnowledgeEra::BronzeAge, &["basic_communication"], "Ability to form social groups with shared goals.", Some("form_group")),
        item("territorial_claim", "Territorial Claim", KnowledgeEra::BronzeAge, &["group_formation"], "Ability to claim ownership of structures and locations.", Some("claim")),
        item("oral_tradition", "Oral Tradition", KnowledgeEra::BronzeAge, &["basic_communication", "group_formation"], "Preservation of knowledge through spoken word.", Some("teach")),
        item("basic_tools", "Basic Tool Crafting", KnowledgeEra::BronzeAge, &["gather_wood", "gather_stone"], "Ability to craft simple tools from wood and stone.", Some("craft (tool)")),
        item("mining", "Mining", KnowledgeEra::BronzeAge, &["basic_tools", "gather_stone"], "Ability to extract ore from rocky terrain.", Some("mine")),
        item("smelting", "Smelting", KnowledgeEra::BronzeAge, &["mining", "build_campfire"], "Ability to convert ore into metal using fire.", Some("smelt")),
        item("metalworking", "Metalworking", KnowledgeEra::BronzeAge, &["smelting", "basic_tools"], "Ability to shape metal into tools and goods.", Some("craft (metal tools)")),
        item("masonry", "Masonry", KnowledgeEra::BronzeAge, &["gather_stone", "build_hut"], "Advanced stone construction techniques.", Some("build well")),
        item("build_forge", "Forge Construction", KnowledgeEra::BronzeAge, &["masonry", "fire_mastery"], "Ability to build a forge for metalworking.", Some("build forge")),
        item("build_workshop", "Workshop Construction", KnowledgeEra::BronzeAge, &["build_hut", "basic_tools"], "Ability to build a workshop for crafting.", Some("build workshop")),
        item("build_market", "Market Construction", KnowledgeEra::BronzeAge, &["barter_system", "build_hut"], "Ability to build a formal trading venue.", Some("build market")),
        item("food_preservation", "Food Preservation", KnowledgeEra::BronzeAge, &["agriculture", "build_storage"], "Techniques for storing food long-term.", None),
        item("herbalism", "Herbalism", KnowledgeEra::BronzeAge, &["basic_medicine", "agriculture"], "Systematic knowledge of medicinal plants.", None),
        item("fiber_working", "Fiber Working", KnowledgeEra::BronzeAge, &["gather_food", "basic_tools"], "Ability to process plant fibers for rope and baskets.", Some("gather (fiber)")),
        item("hide_working", "Hide Working", KnowledgeEra::BronzeAge, &["animal_tracking", "basic_tools"], "Ability to process animal hides for clothing.", Some("craft (hide)")),
    ]
}

/// Build the Medieval-era knowledge items (written language, governance).
fn medieval_foundations() -> Vec<KnowledgeItem> {
    vec![
        item("written_language", "Written Language", KnowledgeEra::Classical, &["oral_tradition", "pottery"], "Ability to record information in written form.", Some("write, read")),
        item("currency_concept", "Currency Concept", KnowledgeEra::Classical, &["barter_system", "written_language"], "Understanding of abstract medium of exchange.", Some("craft (currency_token)")),
        item("governance", "Governance", KnowledgeEra::Classical, &["group_formation", "territorial_claim"], "Systems of collective decision-making.", None),
        item("legislation", "Legislation", KnowledgeEra::Classical, &["governance", "written_language"], "Ability to create formal rules and laws.", Some("legislate")),
        item("build_library", "Library Construction", KnowledgeEra::Classical, &["written_language", "build_hut"], "Ability to build a knowledge repository.", Some("build library")),
        item("build_wall", "Wall Construction", KnowledgeEra::Classical, &["masonry", "territorial_claim"], "Ability to build defensive fortifications.", Some("build wall")),
        item("basic_engineering", "Basic Engineering", KnowledgeEra::Classical, &["masonry", "basic_tools"], "Understanding of structural principles.", None),
        item("bridge_building", "Bridge Building", KnowledgeEra::Classical, &["basic_engineering", "gather_wood"], "Ability to build bridges across obstacles.", Some("build bridge, improve_route")),
        item("organized_labor", "Organized Labor", KnowledgeEra::Classical, &["group_formation", "governance"], "Coordination of group work efforts.", None),
    ]
}

// ---------------------------------------------------------------------------
// Extended Tech Tree: Iron Age through Early Industrial (Task 6.5.1)
// ---------------------------------------------------------------------------

/// Build the Advanced Agriculture items (Iron Age through Early Industrial).
fn advanced_agriculture_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "irrigation", "Irrigation", KnowledgeEra::IronAge,
            &["agriculture", "masonry"],
            "Channeling water to crops through constructed ditches and canals.",
            Some("build irrigation_system"),
        ),
        item(
            "crop_rotation", "Crop Rotation", KnowledgeEra::IronAge,
            &["agriculture", "irrigation"],
            "Alternating crops across seasons to maintain soil fertility and increase yields.",
            None,
        ),
        item(
            "animal_husbandry", "Animal Husbandry", KnowledgeEra::IronAge,
            &["animal_tracking", "agriculture"],
            "Domestication and breeding of animals for labor and resources.",
            Some("gather (meat, hide) improved"),
        ),
        item(
            "draft_animals", "Draft Animals", KnowledgeEra::IronAge,
            &["animal_husbandry"],
            "Training animals to pull loads and assist in fieldwork.",
            Some("improved travel, plowing"),
        ),
        item(
            "plowing", "Plowing", KnowledgeEra::IronAge,
            &["draft_animals", "agriculture"],
            "Using draft animals to turn soil, greatly increasing farm productivity.",
            Some("farm_plant (improved yield)"),
        ),
        item(
            "selective_breeding", "Selective Breeding", KnowledgeEra::Medieval,
            &["animal_husbandry", "crop_rotation"],
            "Intentional breeding of plants and animals for desirable traits.",
            None,
        ),
        item(
            "fertilization", "Fertilization", KnowledgeEra::Medieval,
            &["crop_rotation", "animal_husbandry"],
            "Using animal waste and composting to enrich soil for higher yields.",
            None,
        ),
        item(
            "greenhouse", "Greenhouse Construction", KnowledgeEra::EarlyIndustrial,
            &["fertilization", "glassmaking"],
            "Enclosed structures for year-round crop cultivation regardless of weather.",
            Some("farm_plant (weather-immune)"),
        ),
        item(
            "large_scale_farming", "Large-Scale Farming", KnowledgeEra::EarlyIndustrial,
            &["fertilization", "selective_breeding", "organized_labor"],
            "Industrial-scale agricultural production feeding large populations.",
            Some("farm (mass production)"),
        ),
    ]
}

/// Build the Engineering items.
fn engineering_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "wheel", "Wheel", KnowledgeEra::BronzeAge,
            &["basic_tools", "gather_wood"],
            "Circular device enabling rolling transport and mechanical advantage.",
            None,
        ),
        item(
            "axle", "Axle", KnowledgeEra::IronAge,
            &["wheel", "metalworking"],
            "Central shaft connecting wheels for stable rotation under load.",
            None,
        ),
        item(
            "cart", "Cart", KnowledgeEra::IronAge,
            &["axle", "draft_animals"],
            "Wheeled vehicle for transporting goods, increasing carry capacity.",
            Some("move (increased carry capacity)"),
        ),
        item(
            "roads", "Road Building", KnowledgeEra::Classical,
            &["masonry", "organized_labor"],
            "Constructed pathways of packed earth and stone for reliable travel.",
            Some("improve_route (road)"),
        ),
        item(
            "surveying", "Surveying", KnowledgeEra::Classical,
            &["roads", "mathematics"],
            "Measuring and mapping terrain for construction and planning.",
            None,
        ),
        item(
            "pulleys", "Pulleys", KnowledgeEra::Classical,
            &["wheel", "basic_engineering"],
            "Mechanical device multiplying force for lifting heavy loads.",
            None,
        ),
        item(
            "cranes", "Cranes", KnowledgeEra::Medieval,
            &["pulleys", "metalworking"],
            "Large lifting machines combining pulleys and frames for construction.",
            Some("build (reduced material cost)"),
        ),
        item(
            "architecture", "Architecture", KnowledgeEra::Classical,
            &["masonry", "basic_engineering", "mathematics"],
            "Systematic design of buildings and large structures.",
            Some("build (advanced structures)"),
        ),
        item(
            "aqueducts", "Aqueducts", KnowledgeEra::Classical,
            &["architecture", "irrigation"],
            "Elevated channels carrying water over long distances.",
            Some("water supply at distant locations"),
        ),
        item(
            "advanced_bridges", "Advanced Bridges", KnowledgeEra::Medieval,
            &["architecture", "bridge_building", "cranes"],
            "Large-span stone and metal bridges supporting heavy traffic.",
            Some("improve_route (highway)"),
        ),
    ]
}

/// Build the Science items.
fn science_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "astronomy", "Astronomy", KnowledgeEra::BronzeAge,
            &["observe_seasons", "perceive"],
            "Observation and tracking of celestial bodies and their patterns.",
            None,
        ),
        item(
            "calendar", "Calendar", KnowledgeEra::Classical,
            &["astronomy", "written_language"],
            "Systematic timekeeping based on astronomical cycles.",
            None,
        ),
        item(
            "navigation", "Navigation", KnowledgeEra::IronAge,
            &["astronomy", "basic_tools"],
            "Using celestial bodies and landmarks to determine position and direction.",
            Some("move (reduced travel cost)"),
        ),
        item(
            "mathematics", "Mathematics", KnowledgeEra::Classical,
            &["written_language", "basic_trade"],
            "Abstract reasoning about quantities, shapes, and patterns.",
            None,
        ),
        item(
            "geometry", "Geometry", KnowledgeEra::Classical,
            &["mathematics", "surveying"],
            "Mathematical study of shapes, angles, and spatial relationships.",
            None,
        ),
        item(
            "engineering_principles", "Engineering Principles", KnowledgeEra::Classical,
            &["geometry", "basic_engineering"],
            "Systematic application of mathematics to structural design.",
            None,
        ),
        item(
            "anatomy", "Anatomy", KnowledgeEra::IronAge,
            &["herbalism", "animal_tracking"],
            "Understanding of body structure in humans and animals.",
            None,
        ),
        item(
            "surgery", "Surgery", KnowledgeEra::Medieval,
            &["anatomy", "metalworking"],
            "Invasive medical procedures to repair injuries and remove threats.",
            Some("heal (advanced)"),
        ),
        item(
            "metallurgy", "Metallurgy", KnowledgeEra::IronAge,
            &["metalworking", "fire_mastery"],
            "Scientific understanding of metal properties and alloy creation.",
            None,
        ),
        item(
            "steel", "Steel", KnowledgeEra::Medieval,
            &["metallurgy", "build_forge"],
            "High-carbon iron alloy far stronger than bronze or wrought iron.",
            Some("craft (advanced tools, weapons)"),
        ),
        item(
            "alloys", "Alloys", KnowledgeEra::EarlyIndustrial,
            &["steel", "mathematics"],
            "Deliberate combination of metals to achieve specific properties.",
            Some("craft (specialized materials)"),
        ),
    ]
}

/// Build the Medicine items.
fn medicine_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "antiseptics", "Antiseptics", KnowledgeEra::IronAge,
            &["herbalism", "fire_mastery"],
            "Using heat and herbal compounds to prevent wound infection.",
            Some("heal (reduced infection risk)"),
        ),
        item(
            "bandaging", "Bandaging", KnowledgeEra::IronAge,
            &["antiseptics", "fiber_working"],
            "Wrapping wounds with clean cloth to promote healing.",
            Some("heal (improved recovery)"),
        ),
        item(
            "splinting", "Splinting", KnowledgeEra::IronAge,
            &["bandaging", "basic_tools"],
            "Immobilizing broken bones with rigid supports.",
            Some("heal (injury stabilization)"),
        ),
        item(
            "diagnosis", "Diagnosis", KnowledgeEra::Classical,
            &["anatomy", "herbalism"],
            "Systematic identification of ailments from symptoms.",
            None,
        ),
        item(
            "quarantine", "Quarantine", KnowledgeEra::Classical,
            &["diagnosis", "governance"],
            "Isolation of sick individuals to prevent disease spread.",
            None,
        ),
        item(
            "pharmacology", "Pharmacology", KnowledgeEra::Medieval,
            &["diagnosis", "herbalism", "written_language"],
            "Systematic study and preparation of medicinal compounds.",
            Some("craft (medicine, improved)"),
        ),
        item(
            "public_health", "Public Health", KnowledgeEra::EarlyIndustrial,
            &["quarantine", "sanitation", "pharmacology"],
            "Organized systems for preventing disease at population scale.",
            None,
        ),
    ]
}

/// Build the Manufacturing items.
fn manufacturing_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "kiln", "Kiln", KnowledgeEra::BronzeAge,
            &["pottery", "fire_mastery"],
            "High-temperature furnace for firing ceramics and bricks.",
            Some("craft (improved pottery)"),
        ),
        item(
            "glassmaking", "Glassmaking", KnowledgeEra::IronAge,
            &["kiln", "mining"],
            "Melting sand at extreme temperatures to produce glass.",
            Some("craft (glass)"),
        ),
        item(
            "brickmaking", "Brickmaking", KnowledgeEra::IronAge,
            &["kiln", "masonry"],
            "Firing clay into standardized bricks for durable construction.",
            Some("build (improved durability)"),
        ),
        item(
            "weaving", "Weaving", KnowledgeEra::BronzeAge,
            &["fiber_working", "basic_tools"],
            "Interlacing threads to produce cloth and fabric.",
            Some("craft (textiles)"),
        ),
        item(
            "loom", "Loom", KnowledgeEra::IronAge,
            &["weaving", "build_workshop"],
            "Mechanical frame for faster and more uniform weaving.",
            None,
        ),
        item(
            "textiles", "Textile Production", KnowledgeEra::Medieval,
            &["loom", "selective_breeding"],
            "Large-scale production of cloth from animal and plant fibers.",
            Some("craft (textiles, mass)"),
        ),
        item(
            "dyeing", "Dyeing", KnowledgeEra::Medieval,
            &["textiles", "herbalism"],
            "Coloring fabrics with natural dyes extracted from plants and minerals.",
            Some("craft (dyed textiles)"),
        ),
        item(
            "forging", "Forging", KnowledgeEra::IronAge,
            &["metallurgy", "build_forge"],
            "Shaping metal through controlled heating and hammering.",
            Some("craft (forged tools, weapons)"),
        ),
        item(
            "casting", "Casting", KnowledgeEra::Medieval,
            &["forging", "kiln"],
            "Pouring molten metal into molds for complex shapes.",
            Some("craft (cast metal goods)"),
        ),
        item(
            "milling", "Milling", KnowledgeEra::EarlyIndustrial,
            &["casting", "engineering_principles"],
            "Precision shaping of metal and wood using rotary cutting.",
            Some("craft (precision parts)"),
        ),
    ]
}

/// Build the Infrastructure items.
fn infrastructure_items() -> Vec<KnowledgeItem> {
    vec![
        item(
            "town_planning", "Town Planning", KnowledgeEra::Classical,
            &["roads", "architecture"],
            "Deliberate spatial organization of buildings, roads, and public spaces.",
            None,
        ),
        item(
            "sanitation", "Sanitation", KnowledgeEra::Classical,
            &["aqueducts", "town_planning"],
            "Systems for clean water delivery and waste removal.",
            None,
        ),
        item(
            "sewage", "Sewage Systems", KnowledgeEra::Medieval,
            &["sanitation", "engineering_principles"],
            "Underground channels for carrying waste away from population centers.",
            None,
        ),
        item(
            "record_keeping", "Record Keeping", KnowledgeEra::Classical,
            &["written_language", "mathematics"],
            "Systematic recording of transactions, inventories, and events.",
            None,
        ),
        item(
            "census", "Census", KnowledgeEra::Classical,
            &["record_keeping", "governance"],
            "Periodic counting and categorization of the population.",
            None,
        ),
        item(
            "taxation_system", "Taxation System", KnowledgeEra::Classical,
            &["census", "currency_concept"],
            "Formal system for collecting contributions from the population.",
            None,
        ),
        item(
            "justice_system", "Justice System", KnowledgeEra::Classical,
            &["legislation", "group_formation"],
            "Formal institutions for adjudicating disputes and enforcing laws.",
            Some("enforce (improved)"),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Public builder
// ---------------------------------------------------------------------------

/// Build the complete extended tech tree with all items from Primitive
/// through Early Industrial era.
///
/// Returns approximately 100 knowledge items organized into prerequisite
/// chains. The tree is guaranteed to be a valid DAG with no missing
/// prerequisites.
pub fn build_extended_tech_tree() -> KnowledgeTree {
    let mut all_items = Vec::with_capacity(128);
    all_items.extend(seed_items());
    all_items.extend(bronze_age_items());
    all_items.extend(medieval_foundations());
    all_items.extend(advanced_agriculture_items());
    all_items.extend(engineering_items());
    all_items.extend(science_items());
    all_items.extend(medicine_items());
    all_items.extend(manufacturing_items());
    all_items.extend(infrastructure_items());

    KnowledgeTree::new(all_items)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn tree() -> KnowledgeTree {
        build_extended_tech_tree()
    }

    // --- Structural validation ---

    #[test]
    fn tree_validates_no_errors() {
        let t = tree();
        let errors = t.validate();
        assert!(
            errors.is_empty(),
            "Knowledge tree validation failed: {errors:?}"
        );
    }

    #[test]
    fn tree_has_no_cycles() {
        let t = tree();
        let errors = t.validate();
        let cycle_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.contains("Cycle"))
            .collect();
        assert!(
            cycle_errors.is_empty(),
            "Cycle detected in knowledge tree: {cycle_errors:?}"
        );
    }

    #[test]
    fn all_prerequisites_exist_in_tree() {
        let t = tree();
        for (id, item) in t.iter() {
            for prereq in &item.prerequisites {
                assert!(
                    t.contains(prereq),
                    "Item '{id}' requires '{prereq}' which is not in the tree"
                );
            }
        }
    }

    #[test]
    fn no_self_referencing_prerequisites() {
        let t = tree();
        for (id, item) in t.iter() {
            assert!(
                !item.prerequisites.contains(id),
                "Item '{id}' lists itself as a prerequisite"
            );
        }
    }

    #[test]
    fn tree_has_at_least_90_items() {
        let t = tree();
        assert!(
            t.len() >= 90,
            "Expected at least 90 items, got {}",
            t.len()
        );
    }

    #[test]
    fn tree_has_items_in_all_eras() {
        let t = tree();
        for era in [
            KnowledgeEra::Primitive,
            KnowledgeEra::BronzeAge,
            KnowledgeEra::IronAge,
            KnowledgeEra::Classical,
            KnowledgeEra::Medieval,
            KnowledgeEra::EarlyIndustrial,
        ] {
            let items = t.items_in_era(era);
            assert!(
                !items.is_empty(),
                "No items found in era {era:?}"
            );
        }
    }

    // --- Specific prerequisite chains ---

    #[test]
    fn agriculture_chain_is_correct() {
        let t = tree();
        let agri = t.get("agriculture").unwrap();
        assert!(agri.prerequisites.contains(&String::from("gather_food")));
        assert!(agri.prerequisites.contains(&String::from("observe_seasons")));

        let irrigation = t.get("irrigation").unwrap();
        assert!(irrigation.prerequisites.contains(&String::from("agriculture")));
        assert!(irrigation.prerequisites.contains(&String::from("masonry")));

        let crop_rotation = t.get("crop_rotation").unwrap();
        assert!(crop_rotation.prerequisites.contains(&String::from("agriculture")));
        assert!(crop_rotation.prerequisites.contains(&String::from("irrigation")));
    }

    #[test]
    fn metalworking_chain_is_correct() {
        let t = tree();

        let mining = t.get("mining").unwrap();
        assert!(mining.prerequisites.contains(&String::from("basic_tools")));

        let smelting = t.get("smelting").unwrap();
        assert!(smelting.prerequisites.contains(&String::from("mining")));

        let metalworking = t.get("metalworking").unwrap();
        assert!(metalworking.prerequisites.contains(&String::from("smelting")));

        let metallurgy = t.get("metallurgy").unwrap();
        assert!(metallurgy.prerequisites.contains(&String::from("metalworking")));

        let steel = t.get("steel").unwrap();
        assert!(steel.prerequisites.contains(&String::from("metallurgy")));

        let alloys = t.get("alloys").unwrap();
        assert!(alloys.prerequisites.contains(&String::from("steel")));
    }

    #[test]
    fn writing_to_library_chain() {
        let t = tree();
        let wl = t.get("written_language").unwrap();
        assert!(wl.prerequisites.contains(&String::from("oral_tradition")));
        assert!(wl.prerequisites.contains(&String::from("pottery")));

        let lib = t.get("build_library").unwrap();
        assert!(lib.prerequisites.contains(&String::from("written_language")));
    }

    #[test]
    fn infrastructure_chain() {
        let t = tree();
        let tp = t.get("town_planning").unwrap();
        assert!(tp.prerequisites.contains(&String::from("roads")));
        assert!(tp.prerequisites.contains(&String::from("architecture")));

        let san = t.get("sanitation").unwrap();
        assert!(san.prerequisites.contains(&String::from("aqueducts")));
        assert!(san.prerequisites.contains(&String::from("town_planning")));

        let sew = t.get("sewage").unwrap();
        assert!(sew.prerequisites.contains(&String::from("sanitation")));
    }

    #[test]
    fn medicine_chain() {
        let t = tree();
        let anti = t.get("antiseptics").unwrap();
        assert!(anti.prerequisites.contains(&String::from("herbalism")));

        let band = t.get("bandaging").unwrap();
        assert!(band.prerequisites.contains(&String::from("antiseptics")));

        let splint = t.get("splinting").unwrap();
        assert!(splint.prerequisites.contains(&String::from("bandaging")));
    }

    // --- Era ordering ---

    #[test]
    fn era_ordering_is_monotonic() {
        let t = tree();
        // For every item, its prerequisites should be in the same or
        // earlier era. This validates the tech tree progression.
        for (_id, item) in t.iter() {
            for prereq_id in &item.prerequisites {
                if let Some(prereq) = t.get(prereq_id) {
                    assert!(
                        prereq.era <= item.era,
                        "Item '{}' (era {:?}) has prerequisite '{}' from later era {:?}",
                        item.id, item.era, prereq_id, prereq.era,
                    );
                }
            }
        }
    }

    // --- Discovery candidates ---

    #[test]
    fn blank_slate_agent_can_discover_primitive_items() {
        let t = tree();
        let knowledge: BTreeSet<String> = ["exist", "perceive"]
            .iter()
            .map(|s| String::from(*s))
            .collect();
        let candidates = t.discovery_candidates(&knowledge);
        // Should include things like move, basic_communication, gather_food, etc.
        let candidate_ids: BTreeSet<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
        assert!(candidate_ids.contains("move"), "Should be able to discover 'move'");
        assert!(candidate_ids.contains("basic_communication"), "Should be able to discover 'basic_communication'");
        assert!(candidate_ids.contains("gather_food"), "Should be able to discover 'gather_food'");
        // Should NOT include agriculture (needs observe_seasons + gather_food)
        assert!(!candidate_ids.contains("agriculture"));
    }

    #[test]
    fn advanced_agent_can_discover_industrial_items() {
        let t = tree();
        // Give agent a lot of prerequisite knowledge
        let knowledge: BTreeSet<String> = [
            "exist", "perceive", "move", "basic_communication",
            "gather_food", "gather_wood", "gather_stone", "drink_water",
            "eat", "rest", "build_campfire", "build_lean_to", "basic_trade",
            "observe_seasons", "animal_tracking", "cooking", "fire_mastery",
            "agriculture", "build_hut", "build_storage", "pottery",
            "basic_medicine", "barter_system", "group_formation",
            "territorial_claim", "oral_tradition", "basic_tools",
            "mining", "smelting", "metalworking", "masonry",
            "build_forge", "build_workshop", "herbalism", "fiber_working",
            "hide_working", "written_language", "governance", "legislation",
            "basic_engineering", "organized_labor", "build_library",
            "irrigation", "crop_rotation", "animal_husbandry",
            "draft_animals", "selective_breeding", "fertilization",
            "metallurgy", "steel", "mathematics", "record_keeping",
            "roads", "wheel", "axle", "pulleys", "cranes",
            "architecture", "aqueducts", "town_planning", "sanitation",
            "engineering_principles", "geometry", "surveying",
            "anatomy", "surgery", "antiseptics", "bandaging",
            "splinting", "diagnosis", "quarantine", "pharmacology",
            "kiln", "glassmaking", "casting", "forging",
            "currency_concept", "census", "taxation_system",
            "sewage", "loom", "textiles", "dyeing",
        ].iter().map(|s| String::from(*s)).collect();

        let candidates = t.discovery_candidates(&knowledge);
        let candidate_ids: BTreeSet<&str> = candidates.iter().map(|c| c.id.as_str()).collect();

        // Early Industrial items should now be discoverable
        assert!(
            candidate_ids.contains("greenhouse")
                || candidate_ids.contains("large_scale_farming")
                || candidate_ids.contains("alloys")
                || candidate_ids.contains("milling")
                || candidate_ids.contains("public_health"),
            "Advanced agent should have at least one Early Industrial candidate. Candidates: {candidate_ids:?}"
        );
    }

    #[test]
    fn can_discover_returns_false_for_missing_prereqs() {
        let t = tree();
        let empty: BTreeSet<String> = BTreeSet::new();
        assert!(!t.can_discover("agriculture", &empty));
        assert!(!t.can_discover("steel", &empty));
    }

    #[test]
    fn can_discover_returns_true_with_all_prereqs() {
        let t = tree();
        let knowledge: BTreeSet<String> = ["gather_food", "observe_seasons"]
            .iter()
            .map(|s| String::from(*s))
            .collect();
        assert!(t.can_discover("agriculture", &knowledge));
    }

    #[test]
    fn lookup_nonexistent_item_returns_none() {
        let t = tree();
        assert!(t.get("quantum_computing").is_none());
    }

    // --- Item identity ---

    #[test]
    fn all_ids_are_snake_case() {
        let t = tree();
        for (id, _) in t.iter() {
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "Item ID '{id}' is not snake_case"
            );
        }
    }

    #[test]
    fn no_duplicate_ids() {
        let items = {
            let mut all = Vec::with_capacity(128);
            all.extend(seed_items());
            all.extend(bronze_age_items());
            all.extend(medieval_foundations());
            all.extend(advanced_agriculture_items());
            all.extend(engineering_items());
            all.extend(science_items());
            all.extend(medicine_items());
            all.extend(manufacturing_items());
            all.extend(infrastructure_items());
            all
        };
        let mut seen = BTreeSet::new();
        for item in &items {
            assert!(
                seen.insert(&item.id),
                "Duplicate knowledge item ID: '{}'",
                item.id
            );
        }
    }

    #[test]
    fn all_items_have_descriptions() {
        let t = tree();
        for (id, item) in t.iter() {
            assert!(
                !item.description.is_empty(),
                "Item '{id}' has an empty description"
            );
        }
    }

    #[test]
    fn exist_has_no_prerequisites() {
        let t = tree();
        let exist = t.get("exist").unwrap();
        assert!(exist.prerequisites.is_empty());
    }
}
