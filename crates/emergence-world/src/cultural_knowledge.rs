//! Cultural knowledge system: non-mechanical discoveries that influence agent behavior.
//!
//! Cultural knowledge is distinct from mechanical/technical knowledge. It does not
//! unlock actions or crafting recipes. Instead, it influences agent behavior, social
//! cohesion, group identity, and decision-making.
//!
//! # Categories
//!
//! Cultural knowledge falls into eight categories:
//! - [`Philosophy`](CulturalCategory::Philosophy) -- worldviews and value systems
//! - [`Art`](CulturalCategory::Art) -- creative expression
//! - [`Music`](CulturalCategory::Music) -- musical traditions
//! - [`Mythology`](CulturalCategory::Mythology) -- origin stories and supernatural beliefs
//! - [`Ethics`](CulturalCategory::Ethics) -- moral codes and property norms
//! - [`Tradition`](CulturalCategory::Tradition) -- social customs and ceremonies
//! - [`Ritual`](CulturalCategory::Ritual) -- spiritual practices
//! - [`Language`](CulturalCategory::Language) -- linguistic conventions
//!
//! # Behavioral Influence
//!
//! Each cultural item carries a [`BehavioralInfluence`] that modifies agent
//! tendencies (cooperation, aggression, risk, honesty, industriousness).
//! When aggregated across all cultural knowledge an agent holds, modifiers
//! are averaged and clamped to the range \[-0.5, 0.5\].
//!
//! # Social Cohesion
//!
//! Shared cultural knowledge between two agents produces a social cohesion
//! bonus. The more culture two agents share, the stronger their bond.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use emergence_types::AgentId;

// ---------------------------------------------------------------------------
// Cultural Category
// ---------------------------------------------------------------------------

/// The category of a cultural knowledge item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CulturalCategory {
    /// Worldviews, value systems, and abstract thought.
    Philosophy,
    /// Creative expression through visual media.
    Art,
    /// Musical traditions and sonic expression.
    Music,
    /// Origin stories, legends, and supernatural beliefs.
    Mythology,
    /// Moral codes, property norms, and justice systems.
    Ethics,
    /// Social customs, ceremonies, and rites of passage.
    Tradition,
    /// Spiritual practices and devotional acts.
    Ritual,
    /// Linguistic conventions and communication patterns.
    Language,
}

// ---------------------------------------------------------------------------
// Behavioral Influence
// ---------------------------------------------------------------------------

/// How a cultural item influences agents who hold it.
///
/// Each modifier ranges from -1.0 to 1.0. Positive values increase the
/// tendency, negative values decrease it. When aggregated across multiple
/// cultural items, the final modifiers are clamped to \[-0.5, 0.5\].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehavioralInfluence {
    /// Shifts cooperation tendency. Positive = more cooperative.
    pub cooperation_modifier: f64,
    /// Shifts aggression tendency. Positive = more aggressive.
    pub aggression_modifier: f64,
    /// Shifts risk tolerance. Positive = more risk-taking.
    pub risk_modifier: f64,
    /// Shifts honesty tendency. Positive = more honest.
    pub honesty_modifier: f64,
    /// Shifts industriousness. Positive = more industrious.
    pub industriousness_modifier: f64,
}

impl BehavioralInfluence {
    /// Create a neutral behavioral influence (all modifiers zero).
    pub const fn neutral() -> Self {
        Self {
            cooperation_modifier: 0.0,
            aggression_modifier: 0.0,
            risk_modifier: 0.0,
            honesty_modifier: 0.0,
            industriousness_modifier: 0.0,
        }
    }

    /// Create a behavioral influence, clamping all modifiers to \[-1.0, 1.0\].
    pub fn new(
        cooperation: f64,
        aggression: f64,
        risk: f64,
        honesty: f64,
        industriousness: f64,
    ) -> Self {
        Self {
            cooperation_modifier: clamp_modifier(cooperation, -1.0, 1.0),
            aggression_modifier: clamp_modifier(aggression, -1.0, 1.0),
            risk_modifier: clamp_modifier(risk, -1.0, 1.0),
            honesty_modifier: clamp_modifier(honesty, -1.0, 1.0),
            industriousness_modifier: clamp_modifier(industriousness, -1.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Cultural Knowledge
// ---------------------------------------------------------------------------

/// A single cultural knowledge item.
///
/// Cultural items are created either as predefined seeds or by agents during
/// the simulation. They carry behavioral modifiers and a social cohesion
/// bonus that affects agents who hold them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CulturalKnowledge {
    /// Unique identifier (snake\_case, e.g. `"pacifism"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// The category this item belongs to.
    pub category: CulturalCategory,
    /// Narrative description of the cultural item.
    pub description: String,
    /// The agent who first expressed or created this item (`None` for seeds).
    pub originator: Option<AgentId>,
    /// The tick when this item was first registered.
    pub origin_tick: u64,
    /// How this item affects agents who hold it.
    pub behavioral_influence: BehavioralInfluence,
    /// Bonus to social cohesion between agents who share this item (0.0 to 1.0).
    pub social_cohesion_bonus: f64,
}

// ---------------------------------------------------------------------------
// Aggregate Modifiers (returned from compute)
// ---------------------------------------------------------------------------

/// Aggregated behavioral modifiers for an agent, computed from all held
/// cultural knowledge. Each modifier is clamped to \[-0.5, 0.5\].
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateModifiers {
    /// Aggregate cooperation modifier.
    pub cooperation: f64,
    /// Aggregate aggression modifier.
    pub aggression: f64,
    /// Aggregate risk modifier.
    pub risk: f64,
    /// Aggregate honesty modifier.
    pub honesty: f64,
    /// Aggregate industriousness modifier.
    pub industriousness: f64,
}

impl AggregateModifiers {
    /// A zeroed-out modifier set (no cultural influence).
    pub const fn zero() -> Self {
        Self {
            cooperation: 0.0,
            aggression: 0.0,
            risk: 0.0,
            honesty: 0.0,
            industriousness: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Cultural Registry
// ---------------------------------------------------------------------------

/// Central registry for all cultural knowledge in the simulation.
///
/// Tracks which cultural items exist, which agents hold them, and provides
/// queries for similarity, behavioral aggregation, and cohesion bonuses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CulturalRegistry {
    /// All registered cultural knowledge items, keyed by ID.
    items: BTreeMap<String, CulturalKnowledge>,
    /// Maps agent ID to the set of cultural item IDs they hold.
    agent_culture: HashMap<AgentId, BTreeSet<String>>,
    /// Maps cultural item ID to the set of agents who hold it.
    item_holders: HashMap<String, BTreeSet<AgentId>>,
}

impl CulturalRegistry {
    /// Create an empty cultural registry.
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
            agent_culture: HashMap::new(),
            item_holders: HashMap::new(),
        }
    }

    /// Register a new cultural knowledge item.
    ///
    /// Returns `false` if an item with the same ID already exists.
    pub fn register_cultural_knowledge(&mut self, item: CulturalKnowledge) -> bool {
        if self.items.contains_key(&item.id) {
            return false;
        }
        let id = item.id.clone();
        self.items.insert(id.clone(), item);
        self.item_holders.entry(id).or_default();
        true
    }

    /// Record that an agent has adopted a cultural knowledge item.
    ///
    /// Returns `false` if the item does not exist or the agent already holds it.
    pub fn agent_learns(&mut self, agent_id: AgentId, knowledge_id: &str) -> bool {
        if !self.items.contains_key(knowledge_id) {
            return false;
        }
        let agent_set = self.agent_culture.entry(agent_id).or_default();
        if !agent_set.insert(knowledge_id.to_owned()) {
            return false;
        }
        self.item_holders
            .entry(knowledge_id.to_owned())
            .or_default()
            .insert(agent_id);
        true
    }

    /// Record that an agent has abandoned a cultural knowledge item.
    ///
    /// Returns `false` if the agent did not hold this item.
    pub fn agent_forgets(&mut self, agent_id: AgentId, knowledge_id: &str) -> bool {
        let Some(agent_set) = self.agent_culture.get_mut(&agent_id) else {
            return false;
        };
        if !agent_set.remove(knowledge_id) {
            return false;
        }
        if agent_set.is_empty() {
            self.agent_culture.remove(&agent_id);
        }
        if let Some(holders) = self.item_holders.get_mut(knowledge_id) {
            holders.remove(&agent_id);
        }
        true
    }

    /// Return all cultural item IDs held by an agent.
    pub fn get_agent_culture(&self, agent_id: AgentId) -> BTreeSet<String> {
        self.agent_culture
            .get(&agent_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the cultural item IDs shared between two agents.
    pub fn get_shared_culture(&self, a: AgentId, b: AgentId) -> BTreeSet<String> {
        let set_a = self.get_agent_culture(a);
        let set_b = self.get_agent_culture(b);
        set_a.intersection(&set_b).cloned().collect()
    }

    /// Compute the Jaccard similarity coefficient between two agents' cultural sets.
    ///
    /// Returns 0.0 if both agents hold no cultural knowledge.
    /// Returns 1.0 if they hold exactly the same set.
    pub fn cultural_similarity(&self, a: AgentId, b: AgentId) -> f64 {
        let set_a: HashSet<&String> = self
            .agent_culture
            .get(&a)
            .map(|s| s.iter().collect())
            .unwrap_or_default();
        let set_b: HashSet<&String> = self
            .agent_culture
            .get(&b)
            .map(|s| s.iter().collect())
            .unwrap_or_default();

        let intersection_count = set_a.intersection(&set_b).count();
        let union_count = set_a.union(&set_b).count();

        if union_count == 0 {
            return 0.0;
        }

        // Both values are usize from set operations, safe to convert to f64 for division.
        #[allow(clippy::cast_precision_loss)]
        let similarity = intersection_count as f64 / union_count as f64;
        similarity
    }

    /// Return all cultural items in a given category.
    pub fn get_by_category(&self, category: CulturalCategory) -> Vec<&CulturalKnowledge> {
        self.items
            .values()
            .filter(|item| item.category == category)
            .collect()
    }

    /// Return cultural items sorted by adoption count (most widespread first).
    pub fn most_widespread(&self) -> Vec<(&CulturalKnowledge, usize)> {
        let mut items: Vec<(&CulturalKnowledge, usize)> = self
            .items
            .values()
            .map(|item| {
                let count = self
                    .item_holders
                    .get(&item.id)
                    .map_or(0, BTreeSet::len);
                (item, count)
            })
            .collect();
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items
    }

    /// Compute aggregate behavioral modifiers for an agent.
    ///
    /// Averages the behavioral influences of all cultural knowledge held by
    /// the agent, then clamps each modifier to \[-0.5, 0.5\].
    ///
    /// Returns [`AggregateModifiers::zero()`] if the agent holds no cultural knowledge.
    pub fn compute_behavioral_modifiers(&self, agent_id: AgentId) -> AggregateModifiers {
        let Some(culture_ids) = self.agent_culture.get(&agent_id) else {
            return AggregateModifiers::zero();
        };

        if culture_ids.is_empty() {
            return AggregateModifiers::zero();
        }

        let mut coop_sum = 0.0_f64;
        let mut aggr_sum = 0.0_f64;
        let mut risk_sum = 0.0_f64;
        let mut hone_sum = 0.0_f64;
        let mut indu_sum = 0.0_f64;
        let mut count = 0_u64;

        for kid in culture_ids {
            if let Some(item) = self.items.get(kid) {
                let inf = &item.behavioral_influence;
                coop_sum += inf.cooperation_modifier;
                aggr_sum += inf.aggression_modifier;
                risk_sum += inf.risk_modifier;
                hone_sum += inf.honesty_modifier;
                indu_sum += inf.industriousness_modifier;
                count = count.saturating_add(1);
            }
        }

        if count == 0 {
            return AggregateModifiers::zero();
        }

        // count is guaranteed > 0 and came from a u64 counter, safe to convert.
        #[allow(clippy::cast_precision_loss)]
        let divisor = count as f64;

        AggregateModifiers {
            cooperation: clamp_modifier(coop_sum / divisor, -0.5, 0.5),
            aggression: clamp_modifier(aggr_sum / divisor, -0.5, 0.5),
            risk: clamp_modifier(risk_sum / divisor, -0.5, 0.5),
            honesty: clamp_modifier(hone_sum / divisor, -0.5, 0.5),
            industriousness: clamp_modifier(indu_sum / divisor, -0.5, 0.5),
        }
    }

    /// Compute the social cohesion bonus between two agents from shared culture.
    ///
    /// For each shared cultural item, its `social_cohesion_bonus` is summed.
    /// The result is clamped to \[0.0, 1.0\].
    pub fn social_cohesion_between(&self, a: AgentId, b: AgentId) -> f64 {
        let shared = self.get_shared_culture(a, b);
        let mut total = 0.0_f64;
        for kid in &shared {
            if let Some(item) = self.items.get(kid) {
                total += item.social_cohesion_bonus;
            }
        }
        clamp_modifier(total, 0.0, 1.0)
    }

    /// Return a reference to a cultural knowledge item by ID.
    pub fn get(&self, knowledge_id: &str) -> Option<&CulturalKnowledge> {
        self.items.get(knowledge_id)
    }

    /// Return the total number of registered cultural knowledge items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Return the number of agents who hold a given cultural item.
    pub fn holder_count(&self, knowledge_id: &str) -> usize {
        self.item_holders
            .get(knowledge_id)
            .map_or(0, BTreeSet::len)
    }
}

// ---------------------------------------------------------------------------
// Seed Data
// ---------------------------------------------------------------------------

/// Create a [`CulturalRegistry`] pre-populated with seed cultural items.
///
/// Returns a registry containing ~25 predefined items across all categories.
/// These items have no originator (`None`) and an origin tick of 0.
#[allow(clippy::too_many_lines)]
pub fn seed_cultural_knowledge() -> CulturalRegistry {
    let mut registry = CulturalRegistry::new();

    let seeds: Vec<CulturalKnowledge> = vec![
        // --- Philosophy ---
        CulturalKnowledge {
            id: String::from("pacifism"),
            name: String::from("Pacifism"),
            category: CulturalCategory::Philosophy,
            description: String::from("The belief that violence is never justified and disputes should be resolved through dialogue."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(-0.1, -0.6, -0.3, 0.2, 0.0),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("meritocracy"),
            name: String::from("Meritocracy"),
            category: CulturalCategory::Philosophy,
            description: String::from("The belief that status and reward should follow demonstrated competence and effort."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.0, 0.1, 0.2, 0.1, 0.5),
            social_cohesion_bonus: 0.10,
        },
        CulturalKnowledge {
            id: String::from("collectivism"),
            name: String::from("Collectivism"),
            category: CulturalCategory::Philosophy,
            description: String::from("The group matters more than the individual. Shared resources and collective decision-making."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.6, -0.2, -0.1, 0.1, 0.2),
            social_cohesion_bonus: 0.20,
        },
        CulturalKnowledge {
            id: String::from("individualism"),
            name: String::from("Individualism"),
            category: CulturalCategory::Philosophy,
            description: String::from("Personal freedom and self-reliance above all. Each agent charts their own course."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(-0.3, 0.1, 0.4, 0.0, 0.3),
            social_cohesion_bonus: 0.05,
        },
        CulturalKnowledge {
            id: String::from("stoicism"),
            name: String::from("Stoicism"),
            category: CulturalCategory::Philosophy,
            description: String::from("Endure hardship with calm acceptance. Focus on what you can control."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, -0.3, -0.2, 0.3, 0.4),
            social_cohesion_bonus: 0.10,
        },

        // --- Art ---
        CulturalKnowledge {
            id: String::from("cave_painting"),
            name: String::from("Cave Painting"),
            category: CulturalCategory::Art,
            description: String::from("Visual representation of the world on cave walls, expressing shared experience."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, 0.0, 0.1, 0.0, 0.1),
            social_cohesion_bonus: 0.10,
        },
        CulturalKnowledge {
            id: String::from("pottery_decoration"),
            name: String::from("Pottery Decoration"),
            category: CulturalCategory::Art,
            description: String::from("Artistic patterns applied to clay vessels, creating cultural identity markers."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, 0.0, 0.0, 0.0, 0.2),
            social_cohesion_bonus: 0.12,
        },
        CulturalKnowledge {
            id: String::from("storytelling"),
            name: String::from("Storytelling"),
            category: CulturalCategory::Art,
            description: String::from("Narrative tradition of sharing tales about the past, heroes, and lessons learned."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, 0.0, 0.1, 0.1, 0.0),
            social_cohesion_bonus: 0.18,
        },
        CulturalKnowledge {
            id: String::from("dance"),
            name: String::from("Dance"),
            category: CulturalCategory::Art,
            description: String::from("Rhythmic body movement as expression, celebration, and social bonding."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, -0.1, 0.1, 0.0, 0.0),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("sculpture"),
            name: String::from("Sculpture"),
            category: CulturalCategory::Art,
            description: String::from("Three-dimensional representation of figures and forms from stone or clay."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.0, 0.0, 0.1, 0.0, 0.3),
            social_cohesion_bonus: 0.08,
        },

        // --- Music ---
        CulturalKnowledge {
            id: String::from("drumming"),
            name: String::from("Drumming"),
            category: CulturalCategory::Music,
            description: String::from("Percussive rhythms using hands or sticks on hollow objects."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, 0.1, 0.1, 0.0, 0.1),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("chanting"),
            name: String::from("Chanting"),
            category: CulturalCategory::Music,
            description: String::from("Repetitive vocal patterns used in ceremony and group bonding."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.3, -0.1, 0.0, 0.1, 0.0),
            social_cohesion_bonus: 0.20,
        },
        CulturalKnowledge {
            id: String::from("flute_playing"),
            name: String::from("Flute Playing"),
            category: CulturalCategory::Music,
            description: String::from("Melodic wind instrument carved from bone or reed."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, -0.2, 0.0, 0.0, 0.1),
            social_cohesion_bonus: 0.10,
        },
        CulturalKnowledge {
            id: String::from("work_songs"),
            name: String::from("Work Songs"),
            category: CulturalCategory::Music,
            description: String::from("Rhythmic songs coordinating group labor and making work more bearable."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.3, 0.0, 0.0, 0.0, 0.4),
            social_cohesion_bonus: 0.18,
        },

        // --- Mythology ---
        CulturalKnowledge {
            id: String::from("creation_myth"),
            name: String::from("Creation Myth"),
            category: CulturalCategory::Mythology,
            description: String::from("A shared narrative explaining how the world and its inhabitants came to be."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, 0.0, 0.0, 0.0, 0.0),
            social_cohesion_bonus: 0.20,
        },
        CulturalKnowledge {
            id: String::from("flood_legend"),
            name: String::from("Flood Legend"),
            category: CulturalCategory::Mythology,
            description: String::from("A cautionary tale of a great flood that tests and purifies civilization."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, 0.0, -0.2, 0.1, 0.1),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("ancestor_worship"),
            name: String::from("Ancestor Worship"),
            category: CulturalCategory::Mythology,
            description: String::from("Reverence for deceased forebears, believed to guide and protect the living."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, -0.1, -0.1, 0.2, 0.1),
            social_cohesion_bonus: 0.20,
        },
        CulturalKnowledge {
            id: String::from("spirit_world"),
            name: String::from("Spirit World"),
            category: CulturalCategory::Mythology,
            description: String::from("Belief in an unseen realm of spirits that influence the physical world."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, -0.1, -0.1, 0.1, 0.0),
            social_cohesion_bonus: 0.15,
        },

        // --- Ethics ---
        CulturalKnowledge {
            id: String::from("golden_rule"),
            name: String::from("Golden Rule"),
            category: CulturalCategory::Ethics,
            description: String::from("Treat others as you wish to be treated. Reciprocity as moral foundation."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.4, -0.3, 0.0, 0.5, 0.0),
            social_cohesion_bonus: 0.20,
        },
        CulturalKnowledge {
            id: String::from("eye_for_eye"),
            name: String::from("Eye for an Eye"),
            category: CulturalCategory::Ethics,
            description: String::from("Proportional retribution as justice. Wrongdoers face equal consequence."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(-0.1, 0.3, 0.1, 0.2, 0.0),
            social_cohesion_bonus: 0.10,
        },
        CulturalKnowledge {
            id: String::from("communal_property"),
            name: String::from("Communal Property"),
            category: CulturalCategory::Ethics,
            description: String::from("Resources belong to the community, not individuals. Sharing is the default."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.5, -0.2, -0.1, 0.2, 0.1),
            social_cohesion_bonus: 0.22,
        },
        CulturalKnowledge {
            id: String::from("private_property"),
            name: String::from("Private Property"),
            category: CulturalCategory::Ethics,
            description: String::from("Individuals own the fruits of their labor. Trade is the mechanism of exchange."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(-0.2, 0.1, 0.2, 0.1, 0.4),
            social_cohesion_bonus: 0.08,
        },

        // --- Tradition ---
        CulturalKnowledge {
            id: String::from("burial_rites"),
            name: String::from("Burial Rites"),
            category: CulturalCategory::Tradition,
            description: String::from("Ceremonies honoring the dead, providing closure and cultural continuity."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, -0.1, 0.0, 0.1, 0.0),
            social_cohesion_bonus: 0.18,
        },
        CulturalKnowledge {
            id: String::from("harvest_festival"),
            name: String::from("Harvest Festival"),
            category: CulturalCategory::Tradition,
            description: String::from("Seasonal celebration of abundance, feasting and communal gratitude."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.3, -0.1, 0.1, 0.0, 0.1),
            social_cohesion_bonus: 0.22,
        },
        CulturalKnowledge {
            id: String::from("naming_ceremony"),
            name: String::from("Naming Ceremony"),
            category: CulturalCategory::Tradition,
            description: String::from("Formal ritual welcoming new agents into the community with a chosen name."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, 0.0, 0.0, 0.0, 0.0),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("coming_of_age"),
            name: String::from("Coming of Age"),
            category: CulturalCategory::Tradition,
            description: String::from("A rite marking the transition from youth to adulthood, often involving a trial."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, 0.1, 0.2, 0.1, 0.2),
            social_cohesion_bonus: 0.15,
        },

        // --- Ritual ---
        CulturalKnowledge {
            id: String::from("daily_prayer"),
            name: String::from("Daily Prayer"),
            category: CulturalCategory::Ritual,
            description: String::from("Regular devotional practice providing structure and spiritual grounding."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, -0.2, -0.1, 0.2, 0.2),
            social_cohesion_bonus: 0.15,
        },
        CulturalKnowledge {
            id: String::from("seasonal_offering"),
            name: String::from("Seasonal Offering"),
            category: CulturalCategory::Ritual,
            description: String::from("Periodic sacrifice of resources to appease spirits or express gratitude."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.2, -0.1, 0.0, 0.1, 0.0),
            social_cohesion_bonus: 0.18,
        },
        CulturalKnowledge {
            id: String::from("oath_swearing"),
            name: String::from("Oath Swearing"),
            category: CulturalCategory::Ritual,
            description: String::from("Formal binding promises made before witnesses, enforced by social pressure."),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.1, 0.0, 0.0, 0.4, 0.1),
            social_cohesion_bonus: 0.15,
        },
    ];

    for item in seeds {
        registry.register_cultural_knowledge(item);
    }

    registry
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Clamp a floating-point value to the given range.
fn clamp_modifier(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent() -> AgentId {
        AgentId::new()
    }

    fn make_registry_with_seeds() -> CulturalRegistry {
        seed_cultural_knowledge()
    }

    fn make_simple_item(id: &str, category: CulturalCategory, cohesion: f64) -> CulturalKnowledge {
        CulturalKnowledge {
            id: String::from(id),
            name: String::from(id),
            category,
            description: String::from("test item"),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::neutral(),
            social_cohesion_bonus: cohesion,
        }
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    #[test]
    fn register_cultural_knowledge_succeeds() {
        let mut reg = CulturalRegistry::new();
        let item = make_simple_item("test_item", CulturalCategory::Philosophy, 0.1);
        assert!(reg.register_cultural_knowledge(item));
        assert_eq!(reg.item_count(), 1);
    }

    #[test]
    fn register_duplicate_rejected() {
        let mut reg = CulturalRegistry::new();
        let item = make_simple_item("dup", CulturalCategory::Art, 0.1);
        assert!(reg.register_cultural_knowledge(item));
        let item2 = make_simple_item("dup", CulturalCategory::Art, 0.2);
        assert!(!reg.register_cultural_knowledge(item2));
        assert_eq!(reg.item_count(), 1);
    }

    #[test]
    fn seed_data_populates_registry() {
        let reg = make_registry_with_seeds();
        // We defined 29 seed items.
        assert!(reg.item_count() >= 25);
        // Spot-check a few items exist.
        assert!(reg.get("pacifism").is_some());
        assert!(reg.get("drumming").is_some());
        assert!(reg.get("golden_rule").is_some());
        assert!(reg.get("oath_swearing").is_some());
    }

    // ------------------------------------------------------------------
    // Agent learns / forgets
    // ------------------------------------------------------------------

    #[test]
    fn agent_learns_and_holds_culture() {
        let mut reg = make_registry_with_seeds();
        let agent = make_agent();

        assert!(reg.agent_learns(agent, "pacifism"));
        assert!(reg.agent_learns(agent, "drumming"));

        let culture = reg.get_agent_culture(agent);
        assert_eq!(culture.len(), 2);
        assert!(culture.contains("pacifism"));
        assert!(culture.contains("drumming"));
    }

    #[test]
    fn agent_learns_nonexistent_fails() {
        let mut reg = CulturalRegistry::new();
        let agent = make_agent();
        assert!(!reg.agent_learns(agent, "nonexistent"));
    }

    #[test]
    fn agent_double_learn_fails() {
        let mut reg = make_registry_with_seeds();
        let agent = make_agent();
        assert!(reg.agent_learns(agent, "pacifism"));
        assert!(!reg.agent_learns(agent, "pacifism"));
    }

    #[test]
    fn agent_forgets_removes_culture() {
        let mut reg = make_registry_with_seeds();
        let agent = make_agent();

        assert!(reg.agent_learns(agent, "pacifism"));
        assert!(reg.agent_learns(agent, "drumming"));
        assert!(reg.agent_forgets(agent, "pacifism"));

        let culture = reg.get_agent_culture(agent);
        assert_eq!(culture.len(), 1);
        assert!(!culture.contains("pacifism"));
        assert!(culture.contains("drumming"));
    }

    #[test]
    fn agent_forgets_unheld_fails() {
        let mut reg = make_registry_with_seeds();
        let agent = make_agent();
        assert!(!reg.agent_forgets(agent, "pacifism"));
    }

    // ------------------------------------------------------------------
    // Cultural similarity
    // ------------------------------------------------------------------

    #[test]
    fn cultural_similarity_identical_sets() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(a, "drumming"));
        assert!(reg.agent_learns(b, "pacifism"));
        assert!(reg.agent_learns(b, "drumming"));

        let sim = reg.cultural_similarity(a, b);
        // Jaccard of identical sets = 1.0; use epsilon comparison.
        assert!(sim > 0.99);
    }

    #[test]
    fn cultural_similarity_disjoint_sets() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(b, "drumming"));

        let sim = reg.cultural_similarity(a, b);
        assert!(sim < 0.01);
    }

    #[test]
    fn cultural_similarity_empty_agents() {
        let reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();
        let sim = reg.cultural_similarity(a, b);
        assert!(sim < 0.01);
    }

    #[test]
    fn cultural_similarity_partial_overlap() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        // A: pacifism, drumming, storytelling
        // B: pacifism, drumming, golden_rule
        // Intersection: 2, Union: 4 => Jaccard = 0.5
        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(a, "drumming"));
        assert!(reg.agent_learns(a, "storytelling"));
        assert!(reg.agent_learns(b, "pacifism"));
        assert!(reg.agent_learns(b, "drumming"));
        assert!(reg.agent_learns(b, "golden_rule"));

        let sim = reg.cultural_similarity(a, b);
        assert!(sim > 0.49 && sim < 0.51);
    }

    // ------------------------------------------------------------------
    // Shared culture
    // ------------------------------------------------------------------

    #[test]
    fn get_shared_culture_returns_intersection() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(a, "drumming"));
        assert!(reg.agent_learns(a, "storytelling"));
        assert!(reg.agent_learns(b, "pacifism"));
        assert!(reg.agent_learns(b, "golden_rule"));

        let shared = reg.get_shared_culture(a, b);
        assert_eq!(shared.len(), 1);
        assert!(shared.contains("pacifism"));
    }

    // ------------------------------------------------------------------
    // Behavioral modifiers
    // ------------------------------------------------------------------

    #[test]
    fn behavioral_modifiers_no_culture_returns_zero() {
        let reg = make_registry_with_seeds();
        let agent = make_agent();
        let mods = reg.compute_behavioral_modifiers(agent);
        assert!(mods.cooperation.abs() < f64::EPSILON);
        assert!(mods.aggression.abs() < f64::EPSILON);
        assert!(mods.risk.abs() < f64::EPSILON);
        assert!(mods.honesty.abs() < f64::EPSILON);
        assert!(mods.industriousness.abs() < f64::EPSILON);
    }

    #[test]
    fn behavioral_modifiers_single_item() {
        let mut reg = CulturalRegistry::new();
        let item = CulturalKnowledge {
            id: String::from("test_coop"),
            name: String::from("Test Coop"),
            category: CulturalCategory::Philosophy,
            description: String::from("Test"),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.4, -0.2, 0.0, 0.3, 0.1),
            social_cohesion_bonus: 0.1,
        };
        reg.register_cultural_knowledge(item);

        let agent = make_agent();
        assert!(reg.agent_learns(agent, "test_coop"));

        let mods = reg.compute_behavioral_modifiers(agent);
        // Single item => average = the item's own values, clamped to [-0.5, 0.5].
        assert!((mods.cooperation - 0.4).abs() < 0.001);
        assert!((mods.aggression - (-0.2)).abs() < 0.001);
        assert!(mods.risk.abs() < 0.001);
        assert!((mods.honesty - 0.3).abs() < 0.001);
        assert!((mods.industriousness - 0.1).abs() < 0.001);
    }

    #[test]
    fn behavioral_modifiers_clamped_to_half() {
        let mut reg = CulturalRegistry::new();
        // Two items both with cooperation_modifier = 0.8 => average = 0.8,
        // which should clamp to 0.5.
        let item_a = CulturalKnowledge {
            id: String::from("high_coop_a"),
            name: String::from("High Coop A"),
            category: CulturalCategory::Ethics,
            description: String::from("Test"),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.8, 0.0, 0.0, 0.0, 0.0),
            social_cohesion_bonus: 0.1,
        };
        let item_b = CulturalKnowledge {
            id: String::from("high_coop_b"),
            name: String::from("High Coop B"),
            category: CulturalCategory::Ethics,
            description: String::from("Test"),
            originator: None,
            origin_tick: 0,
            behavioral_influence: BehavioralInfluence::new(0.8, 0.0, 0.0, 0.0, 0.0),
            social_cohesion_bonus: 0.1,
        };
        reg.register_cultural_knowledge(item_a);
        reg.register_cultural_knowledge(item_b);

        let agent = make_agent();
        assert!(reg.agent_learns(agent, "high_coop_a"));
        assert!(reg.agent_learns(agent, "high_coop_b"));

        let mods = reg.compute_behavioral_modifiers(agent);
        // Average = 0.8, clamped to 0.5.
        assert!((mods.cooperation - 0.5).abs() < 0.001);
    }

    // ------------------------------------------------------------------
    // Social cohesion
    // ------------------------------------------------------------------

    #[test]
    fn social_cohesion_no_shared_culture() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(b, "drumming"));

        let cohesion = reg.social_cohesion_between(a, b);
        assert!(cohesion < 0.01);
    }

    #[test]
    fn social_cohesion_with_shared_culture() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        // Both learn pacifism (cohesion_bonus = 0.15) and drumming (0.15).
        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(a, "drumming"));
        assert!(reg.agent_learns(b, "pacifism"));
        assert!(reg.agent_learns(b, "drumming"));

        let cohesion = reg.social_cohesion_between(a, b);
        // Sum = 0.15 + 0.15 = 0.30, clamped to [0, 1].
        assert!((cohesion - 0.3).abs() < 0.01);
    }

    #[test]
    fn social_cohesion_clamped_to_one() {
        let mut reg = CulturalRegistry::new();
        // Create items with huge cohesion bonuses that sum > 1.0.
        for i in 0..5 {
            let id = format!("big_cohesion_{i}");
            let item = CulturalKnowledge {
                id: id.clone(),
                name: id,
                category: CulturalCategory::Tradition,
                description: String::from("test"),
                originator: None,
                origin_tick: 0,
                behavioral_influence: BehavioralInfluence::neutral(),
                social_cohesion_bonus: 0.5,
            };
            reg.register_cultural_knowledge(item);
        }

        let a = make_agent();
        let b = make_agent();
        for i in 0..5 {
            let id = format!("big_cohesion_{i}");
            assert!(reg.agent_learns(a, &id));
            assert!(reg.agent_learns(b, &id));
        }

        let cohesion = reg.social_cohesion_between(a, b);
        // Sum = 5 * 0.5 = 2.5, clamped to 1.0.
        assert!((cohesion - 1.0).abs() < 0.01);
    }

    // ------------------------------------------------------------------
    // Category filtering
    // ------------------------------------------------------------------

    #[test]
    fn get_by_category_returns_correct_items() {
        let reg = make_registry_with_seeds();
        let music = reg.get_by_category(CulturalCategory::Music);
        // We defined 4 music items: drumming, chanting, flute_playing, work_songs.
        assert_eq!(music.len(), 4);
        for item in &music {
            assert_eq!(item.category, CulturalCategory::Music);
        }
    }

    #[test]
    fn get_by_category_philosophy_has_five() {
        let reg = make_registry_with_seeds();
        let philosophy = reg.get_by_category(CulturalCategory::Philosophy);
        assert_eq!(philosophy.len(), 5);
    }

    // ------------------------------------------------------------------
    // Most widespread
    // ------------------------------------------------------------------

    #[test]
    fn most_widespread_ordering() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();
        let c = make_agent();

        // drumming: 3 holders, pacifism: 2 holders, golden_rule: 1 holder.
        assert!(reg.agent_learns(a, "drumming"));
        assert!(reg.agent_learns(b, "drumming"));
        assert!(reg.agent_learns(c, "drumming"));

        assert!(reg.agent_learns(a, "pacifism"));
        assert!(reg.agent_learns(b, "pacifism"));

        assert!(reg.agent_learns(a, "golden_rule"));

        let widespread = reg.most_widespread();
        // First item should be drumming with count 3.
        let first = widespread.first();
        assert!(first.is_some());
        if let Some((item, count)) = first {
            assert_eq!(item.id, "drumming");
            assert_eq!(*count, 3);
        }
    }

    // ------------------------------------------------------------------
    // Holder count
    // ------------------------------------------------------------------

    #[test]
    fn holder_count_tracks_correctly() {
        let mut reg = make_registry_with_seeds();
        let a = make_agent();
        let b = make_agent();

        assert_eq!(reg.holder_count("pacifism"), 0);
        assert!(reg.agent_learns(a, "pacifism"));
        assert_eq!(reg.holder_count("pacifism"), 1);
        assert!(reg.agent_learns(b, "pacifism"));
        assert_eq!(reg.holder_count("pacifism"), 2);
        assert!(reg.agent_forgets(a, "pacifism"));
        assert_eq!(reg.holder_count("pacifism"), 1);
    }
}
