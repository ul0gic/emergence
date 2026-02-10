//! Open innovation proposals for the Emergence simulation.
//!
//! Agents can propose novel inventions by combining their existing
//! knowledge. The [`InnovationEvaluator`] applies rule-based checks
//! to accept, reject, or defer proposals:
//!
//! 1. Are all claimed prerequisite knowledge items actually known?
//! 2. Has this combination already been proposed or registered?
//! 3. Does the combination match a known-good rule in [`COMBINATION_RULES`]?
//! 4. Does it map to an existing tech tree item via keyword matching?
//! 5. If ambiguous, flag for external (LLM) evaluation.
//!
//! Accepted innovations are registered as new [`KnowledgeItem`]s in the
//! tech tree, extending the world's knowledge graph at runtime.

use std::collections::{BTreeMap, BTreeSet};

use emergence_types::AgentId;

use crate::knowledge::{KnowledgeEra, KnowledgeItem, KnowledgeTree};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A proposal from an agent to create a novel invention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnovationProposal {
    /// The agent proposing the innovation.
    pub proposer: AgentId,
    /// The tick when the proposal was submitted.
    pub tick: u64,
    /// Proposed name for the invention.
    pub name: String,
    /// Description of what it does.
    pub description: String,
    /// Knowledge IDs the agent claims to be combining.
    pub combined_knowledge: Vec<String>,
    /// What capability the agent expects the invention to unlock.
    pub intended_benefit: String,
}

/// The result of evaluating an innovation proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnovationResult {
    /// Proposal accepted and registered as a new knowledge item.
    Accepted {
        /// The ID assigned to the new knowledge item.
        new_knowledge_id: String,
        /// The prerequisites for this new item.
        prerequisites: Vec<String>,
    },
    /// Proposal rejected as implausible.
    Rejected {
        /// Why it was rejected.
        reason: String,
    },
    /// The proposal maps to an already existing knowledge item.
    AlreadyExists {
        /// The ID of the existing item.
        existing_id: String,
    },
    /// The proposal is ambiguous and requires external evaluation.
    NeedsEvaluation {
        /// Context for the evaluator.
        context: String,
    },
}

/// The tracked status of a proposal in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    /// Accepted and registered.
    Accepted,
    /// Rejected by the evaluator.
    Rejected,
    /// Pending external evaluation.
    Pending,
    /// Mapped to an existing item.
    AlreadyExists,
}

/// A recorded proposal with its evaluation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalRecord {
    /// The original proposal.
    pub proposal: InnovationProposal,
    /// The evaluation result.
    pub status: ProposalStatus,
    /// The result details.
    pub result: InnovationResult,
}

// ---------------------------------------------------------------------------
// Combination Rules
// ---------------------------------------------------------------------------

/// A known-good combination rule: combining these inputs yields the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinationRule {
    /// Required input knowledge IDs (unordered).
    pub inputs: BTreeSet<String>,
    /// The knowledge ID produced by this combination.
    pub output_id: String,
    /// Display name for the output item.
    pub output_name: String,
    /// Description for the auto-generated item.
    pub description: String,
}

/// Build the set of known-good combination rules.
///
/// These are automatically accepted when an agent proposes a matching
/// combination -- no LLM evaluation needed.
pub fn combination_rules() -> Vec<CombinationRule> {
    vec![
        rule(&["fire_mastery", "gather_stone"], "pottery", "Pottery",
            "Shaping and firing clay into useful vessels."),
        rule(&["agriculture", "irrigation"], "crop_rotation", "Crop Rotation",
            "Alternating crops to maintain soil fertility."),
        rule(&["metallurgy", "build_forge"], "forging", "Forging",
            "Shaping metal through controlled heating and hammering."),
        rule(&["written_language", "mathematics"], "record_keeping", "Record Keeping",
            "Systematic recording of transactions and events."),
        rule(&["anatomy", "herbalism"], "diagnosis", "Diagnosis",
            "Systematic identification of ailments from symptoms."),
        rule(&["wheel", "draft_animals"], "cart", "Cart",
            "Wheeled vehicle for transporting goods."),
        rule(&["masonry", "irrigation"], "aqueducts", "Aqueducts",
            "Elevated channels carrying water over distances."),
        rule(&["gather_food", "build_campfire"], "cooking", "Cooking",
            "Using fire to prepare food for improved nutrition."),
        rule(&["basic_tools", "gather_wood"], "wheel", "Wheel",
            "Circular device enabling rolling transport."),
        rule(&["smelting", "basic_tools"], "metalworking", "Metalworking",
            "Shaping metal into tools and goods."),
        rule(&["kiln", "mining"], "glassmaking", "Glassmaking",
            "Melting sand at extreme temperatures to produce glass."),
        rule(&["governance", "written_language"], "legislation", "Legislation",
            "Creating formal rules and laws."),
        rule(&["herbalism", "fire_mastery"], "antiseptics", "Antiseptics",
            "Using heat and herbal compounds to prevent wound infection."),
        rule(&["fiber_working", "basic_tools"], "weaving", "Weaving",
            "Interlacing threads to produce cloth."),
        rule(&["animal_tracking", "agriculture"], "animal_husbandry", "Animal Husbandry",
            "Domestication and breeding of animals."),
    ]
}

/// Helper to construct a [`CombinationRule`].
fn rule(inputs: &[&str], output_id: &str, output_name: &str, description: &str) -> CombinationRule {
    CombinationRule {
        inputs: inputs.iter().map(|s| String::from(*s)).collect(),
        output_id: String::from(output_id),
        output_name: String::from(output_name),
        description: String::from(description),
    }
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Evaluates and tracks innovation proposals from agents.
///
/// Maintains a registry of all proposals (accepted, rejected, pending)
/// and can register accepted innovations as new knowledge items in the
/// tech tree.
pub struct InnovationEvaluator {
    /// All recorded proposals, keyed by a synthetic index.
    proposals: Vec<ProposalRecord>,
    /// Index from combination hash to proposal indices for dedup.
    combination_index: BTreeMap<String, usize>,
    /// Known-good combination rules.
    rules: Vec<CombinationRule>,
    /// Count of accepted innovations.
    accepted_count: usize,
}

impl InnovationEvaluator {
    /// Create a new evaluator with the default combination rules.
    pub fn new() -> Self {
        Self {
            proposals: Vec::new(),
            combination_index: BTreeMap::new(),
            rules: combination_rules(),
            accepted_count: 0,
        }
    }

    /// Create an evaluator with custom combination rules (for testing).
    pub const fn with_rules(rules: Vec<CombinationRule>) -> Self {
        Self {
            proposals: Vec::new(),
            combination_index: BTreeMap::new(),
            rules,
            accepted_count: 0,
        }
    }

    /// Evaluate an innovation proposal against the knowledge tree and
    /// agent's known items.
    ///
    /// The `agent_knowledge` set should contain the proposing agent's
    /// current knowledge IDs.
    pub fn evaluate_proposal(
        &mut self,
        proposal: InnovationProposal,
        agent_knowledge: &BTreeSet<String>,
        tech_tree: &KnowledgeTree,
    ) -> InnovationResult {
        // Step 1: Check proposer knows all claimed knowledge items.
        for knowledge_id in &proposal.combined_knowledge {
            if !agent_knowledge.contains(knowledge_id.as_str()) {
                let result = InnovationResult::Rejected {
                    reason: format!(
                        "Proposer does not know required knowledge: '{knowledge_id}'"
                    ),
                };
                self.record_proposal(proposal, ProposalStatus::Rejected, result.clone());
                return result;
            }
        }

        // Step 2: Check the combination has not already been proposed.
        let combo_key = Self::combination_key(&proposal.combined_knowledge);
        if let Some(&existing_idx) = self.combination_index.get(&combo_key)
            && let Some(record) = self.proposals.get(existing_idx)
        {
            let result = match &record.status {
                ProposalStatus::Accepted => InnovationResult::AlreadyExists {
                    existing_id: match &record.result {
                        InnovationResult::Accepted { new_knowledge_id, .. } => {
                            new_knowledge_id.clone()
                        }
                        _ => String::from("unknown"),
                    },
                },
                _ => InnovationResult::Rejected {
                    reason: String::from(
                        "This combination of knowledge has already been proposed"
                    ),
                },
            };
            self.record_proposal(
                proposal,
                match &result {
                    InnovationResult::AlreadyExists { .. } => ProposalStatus::AlreadyExists,
                    _ => ProposalStatus::Rejected,
                },
                result.clone(),
            );
            return result;
        }

        // Step 3: Check if the proposal maps to an existing tech tree item.
        if let Some(existing_id) = Self::keyword_match_existing(&proposal, tech_tree) {
            let result = InnovationResult::AlreadyExists { existing_id };
            self.record_proposal(proposal, ProposalStatus::AlreadyExists, result.clone());
            return result;
        }

        // Step 4: Check known-good combination rules.
        let input_set: BTreeSet<String> = proposal.combined_knowledge.iter().cloned().collect();
        for rule in &self.rules {
            if rule.inputs == input_set && !tech_tree.contains(&rule.output_id) {
                let result = InnovationResult::Accepted {
                    new_knowledge_id: rule.output_id.clone(),
                    prerequisites: proposal.combined_knowledge.clone(),
                };
                self.record_proposal(proposal, ProposalStatus::Accepted, result.clone());
                self.accepted_count = self.accepted_count.saturating_add(1);
                return result;
            }
        }

        // Step 5: Check if the combination has at least 2 items and they
        // are plausible (all exist in the tree). If the combination looks
        // reasonable but is not in our rule set, flag for evaluation.
        if proposal.combined_knowledge.len() < 2 {
            let result = InnovationResult::Rejected {
                reason: String::from(
                    "Innovation requires combining at least 2 knowledge items"
                ),
            };
            self.record_proposal(proposal, ProposalStatus::Rejected, result.clone());
            return result;
        }

        // Check all combined items exist in the tech tree.
        let all_in_tree = proposal
            .combined_knowledge
            .iter()
            .all(|id| tech_tree.contains(id));

        if !all_in_tree {
            let result = InnovationResult::Rejected {
                reason: String::from(
                    "One or more combined knowledge items do not exist in the tech tree"
                ),
            };
            self.record_proposal(proposal, ProposalStatus::Rejected, result.clone());
            return result;
        }

        // Ambiguous -- needs external evaluation.
        let context = format!(
            "Agent proposes '{}': combining {:?} for benefit: '{}'. Description: '{}'",
            proposal.name,
            proposal.combined_knowledge,
            proposal.intended_benefit,
            proposal.description,
        );
        let result = InnovationResult::NeedsEvaluation { context };
        self.record_proposal(proposal, ProposalStatus::Pending, result.clone());
        result
    }

    /// Register an accepted innovation into the tech tree.
    ///
    /// Creates a new [`KnowledgeItem`] and inserts it into the tree.
    /// Returns `true` if the item was newly inserted.
    pub fn register_innovation(
        tech_tree: &mut KnowledgeTree,
        knowledge_id: &str,
        name: &str,
        description: &str,
        prerequisites: &[String],
        unlocks: Option<&str>,
    ) -> bool {
        let item = KnowledgeItem {
            id: String::from(knowledge_id),
            name: String::from(name),
            era: KnowledgeEra::EarlyIndustrial,
            prerequisites: prerequisites.to_vec(),
            description: String::from(description),
            unlocks: unlocks.map(String::from),
        };
        tech_tree.insert(item)
    }

    /// Get all proposals by a specific agent.
    pub fn get_innovations_by_agent(&self, agent_id: AgentId) -> Vec<&ProposalRecord> {
        self.proposals
            .iter()
            .filter(|r| r.proposal.proposer == agent_id)
            .collect()
    }

    /// Get all recorded proposals.
    pub fn get_all_innovations(&self) -> &[ProposalRecord] {
        &self.proposals
    }

    /// Return the total count of accepted innovations.
    pub const fn innovation_count(&self) -> usize {
        self.accepted_count
    }

    /// Return the total count of all proposals (any status).
    pub const fn total_proposals(&self) -> usize {
        self.proposals.len()
    }

    // --- Private helpers ---

    /// Record a proposal in the registry.
    fn record_proposal(
        &mut self,
        proposal: InnovationProposal,
        status: ProposalStatus,
        result: InnovationResult,
    ) {
        let combo_key = Self::combination_key(&proposal.combined_knowledge);
        let idx = self.proposals.len();
        // Only store the first occurrence per combination.
        self.combination_index.entry(combo_key).or_insert(idx);
        self.proposals.push(ProposalRecord {
            proposal,
            status,
            result,
        });
    }

    /// Create a deterministic key from a set of knowledge IDs for dedup.
    fn combination_key(knowledge_ids: &[String]) -> String {
        let mut sorted: Vec<&str> = knowledge_ids.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        sorted.join("+")
    }

    /// Attempt to match a proposal to an existing tech tree item via
    /// keyword matching on the proposal name and description.
    fn keyword_match_existing(
        proposal: &InnovationProposal,
        tech_tree: &KnowledgeTree,
    ) -> Option<String> {
        let proposal_name_lower = proposal.name.to_lowercase();
        let proposal_desc_lower = proposal.description.to_lowercase();

        for (id, item) in tech_tree.iter() {
            let item_name_lower = item.name.to_lowercase();
            let item_id_lower = id.to_lowercase();

            // Exact name match.
            if proposal_name_lower == item_name_lower {
                return Some(id.clone());
            }

            // Proposal name contains the item's ID (e.g. "crop_rotation" in
            // "improved crop_rotation system").
            let id_words = item_id_lower.replace('_', " ");
            if proposal_name_lower.contains(&id_words) || proposal_name_lower.contains(&item_id_lower) {
                return Some(id.clone());
            }

            // Proposal description contains the item name.
            if proposal_desc_lower.contains(&item_name_lower)
                && item_name_lower.len() >= 5
            {
                return Some(id.clone());
            }
        }
        None
    }
}

impl Default for InnovationEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use emergence_types::AgentId;

    use super::*;
    use crate::knowledge::build_extended_tech_tree;

    fn agent() -> AgentId {
        AgentId::new()
    }

    fn basic_knowledge() -> BTreeSet<String> {
        [
            "exist", "perceive", "move", "basic_communication",
            "gather_food", "gather_wood", "gather_stone", "drink_water",
            "eat", "rest", "build_campfire", "build_lean_to", "basic_trade",
            "observe_seasons", "animal_tracking", "cooking", "fire_mastery",
            "agriculture", "build_hut", "build_storage", "pottery",
            "basic_medicine", "group_formation", "oral_tradition",
            "basic_tools", "mining", "smelting", "metalworking",
            "masonry", "herbalism", "fiber_working",
        ]
        .iter()
        .map(|s| String::from(*s))
        .collect()
    }

    // --- Valid proposal accepted via combination rule ---

    #[test]
    fn valid_proposal_accepted_via_rule() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();

        // Agent knows anatomy and herbalism
        let mut knowledge = basic_knowledge();
        knowledge.insert(String::from("anatomy"));

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Medical Diagnosis"),
            description: String::from("Identifying ailments from symptoms"),
            combined_knowledge: vec![
                String::from("anatomy"),
                String::from("herbalism"),
            ],
            intended_benefit: String::from("Better healthcare"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            // Could be AlreadyExists if diagnosis is already in the tree
            InnovationResult::AlreadyExists { existing_id } => {
                assert_eq!(existing_id, "diagnosis");
            }
            InnovationResult::Accepted { new_knowledge_id, .. } => {
                assert_eq!(new_knowledge_id, "diagnosis");
            }
            other => panic!("Expected Accepted or AlreadyExists, got {other:?}"),
        }
    }

    // --- Proposal rejected: missing prereqs ---

    #[test]
    fn proposal_rejected_missing_knowledge() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();

        // Agent does NOT know anatomy
        let knowledge = basic_knowledge();

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Advanced Surgery"),
            description: String::from("Complex medical procedures"),
            combined_knowledge: vec![
                String::from("anatomy"),
                String::from("herbalism"),
            ],
            intended_benefit: String::from("Saving lives"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            InnovationResult::Rejected { reason } => {
                assert!(reason.contains("anatomy"), "Reason should mention the missing knowledge: {reason}");
            }
            other => panic!("Expected Rejected, got {other:?}"),
        }
    }

    // --- Duplicate detection ---

    #[test]
    fn duplicate_proposal_rejected() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let knowledge = basic_knowledge();

        let proposal1 = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("New Technique"),
            description: String::from("Combining fire and stone"),
            combined_knowledge: vec![
                String::from("fire_mastery"),
                String::from("gather_stone"),
            ],
            intended_benefit: String::from("Better materials"),
        };

        // First proposal
        let _result1 = evaluator.evaluate_proposal(proposal1, &knowledge, &tree);

        let proposal2 = InnovationProposal {
            proposer,
            tick: 101,
            name: String::from("Another Name"),
            description: String::from("Same combination"),
            combined_knowledge: vec![
                String::from("gather_stone"),
                String::from("fire_mastery"),
            ],
            intended_benefit: String::from("Same thing"),
        };

        // Second proposal with same combination should be detected
        let result2 = evaluator.evaluate_proposal(proposal2, &knowledge, &tree);

        match result2 {
            InnovationResult::AlreadyExists { .. } | InnovationResult::Rejected { .. } => {
                // Expected -- either already accepted or duplicate rejection
            }
            other => panic!("Expected AlreadyExists or Rejected for duplicate, got {other:?}"),
        }
    }

    // --- Already exists mapping ---

    #[test]
    fn proposal_maps_to_existing_item() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let knowledge = basic_knowledge();

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Agriculture"),
            description: String::from("Growing crops from seeds"),
            combined_knowledge: vec![
                String::from("gather_food"),
                String::from("observe_seasons"),
            ],
            intended_benefit: String::from("Food production"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            InnovationResult::AlreadyExists { existing_id } => {
                assert_eq!(existing_id, "agriculture");
            }
            other => panic!("Expected AlreadyExists, got {other:?}"),
        }
    }

    // --- Combination rules fire correctly ---

    #[test]
    fn combination_rule_cooking_fires() {
        // Remove cooking from the tree by using a small custom tree
        let mut tree = crate::knowledge::KnowledgeTree::new(vec![
            crate::knowledge::KnowledgeItem {
                id: String::from("gather_food"),
                name: String::from("Food Gathering"),
                era: KnowledgeEra::Primitive,
                prerequisites: vec![],
                description: String::from("Gathering food."),
                unlocks: None,
            },
            crate::knowledge::KnowledgeItem {
                id: String::from("build_campfire"),
                name: String::from("Campfire Building"),
                era: KnowledgeEra::Primitive,
                prerequisites: vec![],
                description: String::from("Building fires."),
                unlocks: None,
            },
        ]);
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let knowledge: BTreeSet<String> = ["gather_food", "build_campfire"]
            .iter()
            .map(|s| String::from(*s))
            .collect();

        let proposal = InnovationProposal {
            proposer,
            tick: 50,
            name: String::from("Heat Treatment of Food"),
            description: String::from("Applying fire to raw food"),
            combined_knowledge: vec![
                String::from("gather_food"),
                String::from("build_campfire"),
            ],
            intended_benefit: String::from("Better nutrition"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match &result {
            InnovationResult::Accepted { new_knowledge_id, prerequisites } => {
                assert_eq!(new_knowledge_id, "cooking");
                assert!(prerequisites.contains(&String::from("gather_food")));
                assert!(prerequisites.contains(&String::from("build_campfire")));
            }
            other => panic!("Expected Accepted for cooking rule, got {other:?}"),
        }

        // Now register it
        InnovationEvaluator::register_innovation(
            &mut tree,
            "cooking",
            "Cooking",
            "Using fire to prepare food.",
            &[String::from("gather_food"), String::from("build_campfire")],
            Some("craft (cooked food)"),
        );
        assert!(tree.contains("cooking"));
    }

    // --- Needs evaluation (ambiguous) ---

    #[test]
    fn ambiguous_proposal_needs_evaluation() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let knowledge = basic_knowledge();

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Mystical Water Purification"),
            description: String::from("A novel approach to clean water"),
            combined_knowledge: vec![
                String::from("herbalism"),
                String::from("pottery"),
            ],
            intended_benefit: String::from("Clean drinking water"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            InnovationResult::NeedsEvaluation { context } => {
                assert!(context.contains("Mystical Water Purification"));
                assert!(context.contains("herbalism"));
                assert!(context.contains("pottery"));
            }
            other => panic!("Expected NeedsEvaluation for ambiguous proposal, got {other:?}"),
        }
    }

    // --- Too few knowledge items ---

    #[test]
    fn proposal_rejected_too_few_items() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let knowledge = basic_knowledge();

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Super Gathering"),
            description: String::from("Better gathering"),
            combined_knowledge: vec![String::from("gather_food")],
            intended_benefit: String::from("More food"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            InnovationResult::Rejected { reason } => {
                assert!(reason.contains("at least 2"), "Reason: {reason}");
            }
            other => panic!("Expected Rejected for single item, got {other:?}"),
        }
    }

    // --- Nonexistent knowledge in combination ---

    #[test]
    fn proposal_rejected_nonexistent_tree_item() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let proposer = agent();
        let mut knowledge = basic_knowledge();
        knowledge.insert(String::from("quantum_physics"));

        let proposal = InnovationProposal {
            proposer,
            tick: 100,
            name: String::from("Teleportation"),
            description: String::from("Instant travel"),
            combined_knowledge: vec![
                String::from("quantum_physics"),
                String::from("gather_food"),
            ],
            intended_benefit: String::from("Fast travel"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);

        match result {
            InnovationResult::Rejected { reason } => {
                assert!(reason.contains("do not exist"), "Reason: {reason}");
            }
            other => panic!("Expected Rejected for nonexistent tree items, got {other:?}"),
        }
    }

    // --- Agent tracking ---

    #[test]
    fn get_innovations_by_agent_works() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let agent_a = agent();
        let agent_b = agent();
        let knowledge = basic_knowledge();

        // Agent A proposes
        let proposal_a = InnovationProposal {
            proposer: agent_a,
            tick: 100,
            name: String::from("Idea A"),
            description: String::from("Something novel"),
            combined_knowledge: vec![
                String::from("herbalism"),
                String::from("pottery"),
            ],
            intended_benefit: String::from("Benefit A"),
        };
        let _ = evaluator.evaluate_proposal(proposal_a, &knowledge, &tree);

        // Agent B proposes
        let proposal_b = InnovationProposal {
            proposer: agent_b,
            tick: 101,
            name: String::from("Idea B"),
            description: String::from("Something else"),
            combined_knowledge: vec![
                String::from("gather_food"),
                String::from("fire_mastery"),
            ],
            intended_benefit: String::from("Benefit B"),
        };
        let _ = evaluator.evaluate_proposal(proposal_b, &knowledge, &tree);

        let a_proposals = evaluator.get_innovations_by_agent(agent_a);
        assert_eq!(a_proposals.len(), 1);
        assert_eq!(a_proposals.first().unwrap().proposal.proposer, agent_a);

        let b_proposals = evaluator.get_innovations_by_agent(agent_b);
        assert_eq!(b_proposals.len(), 1);
        assert_eq!(b_proposals.first().unwrap().proposal.proposer, agent_b);
    }

    // --- Innovation count ---

    #[test]
    fn innovation_count_tracks_accepted() {
        // Use a minimal tree that does NOT contain "cooking" so the rule fires.
        let tree = crate::knowledge::KnowledgeTree::new(vec![
            crate::knowledge::KnowledgeItem {
                id: String::from("gather_food"),
                name: String::from("Food Gathering"),
                era: KnowledgeEra::Primitive,
                prerequisites: vec![],
                description: String::from("Gathering food."),
                unlocks: None,
            },
            crate::knowledge::KnowledgeItem {
                id: String::from("build_campfire"),
                name: String::from("Campfire Building"),
                era: KnowledgeEra::Primitive,
                prerequisites: vec![],
                description: String::from("Building fires."),
                unlocks: None,
            },
        ]);
        let mut evaluator = InnovationEvaluator::new();
        assert_eq!(evaluator.innovation_count(), 0);

        let knowledge: BTreeSet<String> = ["gather_food", "build_campfire"]
            .iter()
            .map(|s| String::from(*s))
            .collect();

        let proposal = InnovationProposal {
            proposer: agent(),
            tick: 50,
            name: String::from("Heated Food"),
            description: String::from("Cooking with fire"),
            combined_knowledge: vec![
                String::from("gather_food"),
                String::from("build_campfire"),
            ],
            intended_benefit: String::from("Nutrition"),
        };

        let result = evaluator.evaluate_proposal(proposal, &knowledge, &tree);
        assert!(matches!(result, InnovationResult::Accepted { .. }));
        assert_eq!(evaluator.innovation_count(), 1);
    }

    // --- Register innovation ---

    #[test]
    fn register_innovation_adds_to_tree() {
        let mut tree = build_extended_tech_tree();
        let initial_count = tree.len();

        let inserted = InnovationEvaluator::register_innovation(
            &mut tree,
            "quantum_weaving",
            "Quantum Weaving",
            "A novel textile technique",
            &[String::from("weaving"), String::from("mathematics")],
            Some("craft (quantum textiles)"),
        );

        assert!(inserted);
        assert_eq!(tree.len(), initial_count.wrapping_add(1));
        assert!(tree.contains("quantum_weaving"));

        let item = tree.get("quantum_weaving").unwrap();
        assert_eq!(item.name, "Quantum Weaving");
        assert!(item.prerequisites.contains(&String::from("weaving")));
        assert!(item.prerequisites.contains(&String::from("mathematics")));
    }

    // --- Get all innovations ---

    #[test]
    fn get_all_innovations_returns_all() {
        let tree = build_extended_tech_tree();
        let mut evaluator = InnovationEvaluator::new();
        let knowledge = basic_knowledge();

        for i in 0_u64..3 {
            let proposal = InnovationProposal {
                proposer: agent(),
                tick: i,
                name: format!("Idea {i}"),
                description: format!("Novel idea number {i}"),
                combined_knowledge: vec![
                    String::from("herbalism"),
                    String::from("masonry"),
                ],
                intended_benefit: format!("Benefit {i}"),
            };
            let _ = evaluator.evaluate_proposal(proposal, &knowledge, &tree);
        }

        // First proposal is novel, subsequent are duplicates, but all recorded.
        assert_eq!(evaluator.get_all_innovations().len(), 3);
        assert_eq!(evaluator.total_proposals(), 3);
    }

    // --- Combination rules coverage ---

    #[test]
    fn all_combination_rules_have_valid_inputs() {
        let tree = build_extended_tech_tree();
        let rules = combination_rules();
        for rule in &rules {
            for input in &rule.inputs {
                assert!(
                    tree.contains(input),
                    "Combination rule output '{}' has input '{}' not in tech tree",
                    rule.output_id, input,
                );
            }
        }
    }

    #[test]
    fn combination_rules_count() {
        let rules = combination_rules();
        assert!(
            rules.len() >= 10,
            "Expected at least 10 combination rules, got {}",
            rules.len()
        );
    }
}
