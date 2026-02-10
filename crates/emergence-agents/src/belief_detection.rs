//! Belief and narrative detection for the Emergence simulation.
//!
//! Analyzes agent communications and broadcasts to detect emergent belief
//! systems. When multiple agents repeatedly use thematic keywords (spiritual,
//! philosophical, narrative), the detector clusters those themes and flags
//! potential new social constructs. This module implements task 6.4.2 from
//! the build plan.
//!
//! # Detection Pipeline
//!
//! 1. **Record**: each agent communication is scanned for thematic keywords.
//! 2. **Cluster**: keywords that co-occur across agents form `BeliefTheme`s.
//! 3. **Threshold**: when 3+ agents share 2+ themes, it qualifies as a
//!    potential new social construct.
//! 4. **Schism detection**: if a belief system has two sub-clusters with
//!    diverging themes, flag a potential schism.
//!
//! # Keyword Categories
//!
//! - **Spiritual**: god, divine, spirit, sacred, holy, worship, pray, soul,
//!   afterlife, creation
//! - **Philosophical**: truth, justice, freedom, duty, honor, wisdom, virtue,
//!   purpose, meaning
//! - **Narrative**: story, legend, ancestor, prophecy, destiny, chosen,
//!   forbidden

use std::collections::{BTreeMap, HashMap, HashSet};

use emergence_types::AgentId;

// ---------------------------------------------------------------------------
// Keyword categories
// ---------------------------------------------------------------------------

/// Spiritual keywords that indicate religious or supernatural themes.
const SPIRITUAL_KEYWORDS: &[&str] = &[
    "god", "divine", "spirit", "sacred", "holy", "worship", "pray", "soul",
    "afterlife", "creation",
];

/// Philosophical keywords that indicate abstract reasoning themes.
const PHILOSOPHICAL_KEYWORDS: &[&str] = &[
    "truth", "justice", "freedom", "duty", "honor", "wisdom", "virtue",
    "purpose", "meaning",
];

/// Narrative keywords that indicate shared storytelling themes.
const NARRATIVE_KEYWORDS: &[&str] = &[
    "story", "legend", "ancestor", "prophecy", "destiny", "chosen",
    "forbidden",
];

/// Minimum number of agents sharing themes to qualify as a belief system.
const MIN_ADHERENTS_FOR_CONSTRUCT: usize = 3;

/// Minimum number of shared themes to qualify as a belief system.
const MIN_SHARED_THEMES_FOR_CONSTRUCT: usize = 2;

// ---------------------------------------------------------------------------
// Union-Find helpers (module-level to satisfy items_after_statements)
// ---------------------------------------------------------------------------

/// Find the root of element `i` in the union-find `parent` slice,
/// with path compression.
fn uf_find(parent: &mut [usize], mut i: usize) -> usize {
    while let Some(&p) = parent.get(i) {
        if p == i {
            break;
        }
        // Path compression: set parent to grandparent
        let grandparent = parent.get(p).copied().unwrap_or(p);
        if let Some(slot) = parent.get_mut(i) {
            *slot = grandparent;
        }
        i = grandparent;
    }
    i
}

/// Union two sets in the union-find `parent` slice.
fn uf_union(parent: &mut [usize], a: usize, b: usize) {
    let ra = uf_find(parent, a);
    let rb = uf_find(parent, b);
    if ra != rb
        && let Some(slot) = parent.get_mut(rb)
    {
        *slot = ra;
    }
}

// ---------------------------------------------------------------------------
// BeliefTheme
// ---------------------------------------------------------------------------

/// A detected thematic keyword pattern across agent communications.
///
/// Tracks which agents have used which keywords and when the theme
/// was first observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeliefTheme {
    /// The keywords that define this theme.
    pub keywords: Vec<String>,
    /// The tick when this theme was first observed.
    pub first_seen_tick: u64,
    /// Total number of mentions across all agents.
    pub mention_count: u32,
    /// Agents who have used keywords from this theme.
    pub adherent_ids: HashSet<AgentId>,
}

// ---------------------------------------------------------------------------
// DetectedBelief
// ---------------------------------------------------------------------------

/// A belief system that meets the detection threshold.
///
/// Returned by `check_for_new_constructs` when enough agents share
/// enough thematic keywords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedBelief {
    /// The shared keywords that define this belief system.
    pub shared_keywords: Vec<String>,
    /// The agents who share these keywords.
    pub adherent_ids: HashSet<AgentId>,
    /// The tick when the first relevant keyword was observed.
    pub first_seen_tick: u64,
}

// ---------------------------------------------------------------------------
// SchismRisk
// ---------------------------------------------------------------------------

/// A detected risk of schism within a belief system.
///
/// Occurs when a belief's adherents form two sub-clusters with
/// diverging keyword sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchismRisk {
    /// The keywords shared by the primary faction.
    pub primary_keywords: Vec<String>,
    /// Agents in the primary faction.
    pub primary_agents: HashSet<AgentId>,
    /// The keywords shared by the divergent faction.
    pub divergent_keywords: Vec<String>,
    /// Agents in the divergent faction.
    pub divergent_agents: HashSet<AgentId>,
}

// ---------------------------------------------------------------------------
// BeliefDetector
// ---------------------------------------------------------------------------

/// Analyzes agent communications to detect emergent belief systems.
///
/// Maintains per-keyword statistics (which agents mentioned it, how
/// many times, when it was first seen) and provides methods to cluster
/// keywords into belief systems and detect schism risks.
#[derive(Debug, Clone)]
pub struct BeliefDetector {
    /// Per-keyword: set of agents who have used this keyword.
    keyword_agents: HashMap<String, HashSet<AgentId>>,
    /// Per-keyword: total mention count.
    keyword_counts: HashMap<String, u32>,
    /// Per-keyword: tick when first observed.
    keyword_first_seen: HashMap<String, u64>,
    /// Per-agent: set of keywords they have used.
    agent_keywords: BTreeMap<AgentId, HashSet<String>>,
}

impl BeliefDetector {
    /// Create a new empty belief detector.
    pub fn new() -> Self {
        Self {
            keyword_agents: HashMap::new(),
            keyword_counts: HashMap::new(),
            keyword_first_seen: HashMap::new(),
            agent_keywords: BTreeMap::new(),
        }
    }

    /// Record a communication from an agent and extract thematic keywords.
    ///
    /// Scans the message text for keywords from the spiritual, philosophical,
    /// and narrative categories. Updates per-keyword and per-agent tracking.
    pub fn record_communication(
        &mut self,
        agent_id: AgentId,
        tick: u64,
        message: &str,
    ) {
        let lower = message.to_lowercase();

        let all_keywords = SPIRITUAL_KEYWORDS
            .iter()
            .chain(PHILOSOPHICAL_KEYWORDS.iter())
            .chain(NARRATIVE_KEYWORDS.iter());

        for &keyword in all_keywords {
            if lower.contains(keyword) {
                let key = String::from(keyword);

                // Update keyword -> agents mapping
                self.keyword_agents
                    .entry(key.clone())
                    .or_default()
                    .insert(agent_id);

                // Update keyword count
                let count = self.keyword_counts.entry(key.clone()).or_insert(0);
                *count = count.saturating_add(1);

                // Update first seen tick
                self.keyword_first_seen
                    .entry(key.clone())
                    .or_insert(tick);

                // Update agent -> keywords mapping
                self.agent_keywords
                    .entry(agent_id)
                    .or_default()
                    .insert(key);
            }
        }
    }

    /// Cluster thematic keywords into belief themes based on agent overlap.
    ///
    /// Two keywords belong to the same cluster if they share at least one
    /// adherent agent. Returns a list of `BeliefTheme` sorted by adherent
    /// count (descending).
    pub fn detect_clusters(&self) -> Vec<BeliefTheme> {
        let keywords: Vec<String> = self.keyword_agents.keys().cloned().collect();
        let keyword_count = keywords.len();

        // parent[i] = parent index in union-find
        let mut parent: Vec<usize> = (0..keyword_count).collect();

        // For each pair of keywords, check if they share agents
        for i in 0..keyword_count {
            for j in (i.saturating_add(1))..keyword_count {
                let kw_i = keywords.get(i);
                let kw_j = keywords.get(j);

                if let (Some(ki), Some(kj)) = (kw_i, kw_j) {
                    let agents_i = self.keyword_agents.get(ki);
                    let agents_j = self.keyword_agents.get(kj);

                    if let (Some(ai), Some(aj)) = (agents_i, agents_j)
                        && ai.iter().any(|a| aj.contains(a))
                    {
                        uf_union(&mut parent, i, j);
                    }
                }
            }
        }

        // Group keywords by cluster root
        let mut clusters: HashMap<usize, Vec<String>> = HashMap::new();
        for (i, kw) in keywords.iter().enumerate() {
            let root = uf_find(&mut parent, i);
            clusters.entry(root).or_default().push(kw.clone());
        }

        // Build BeliefTheme for each cluster
        let mut themes: Vec<BeliefTheme> = Vec::new();
        for cluster_keywords in clusters.into_values() {
            let mut adherents = HashSet::new();
            let mut total_mentions: u32 = 0;
            let mut first_seen: u64 = u64::MAX;

            for kw in &cluster_keywords {
                if let Some(agents) = self.keyword_agents.get(kw) {
                    for agent in agents {
                        adherents.insert(*agent);
                    }
                }
                if let Some(&count) = self.keyword_counts.get(kw) {
                    total_mentions = total_mentions.saturating_add(count);
                }
                if let Some(&tick) = self.keyword_first_seen.get(kw)
                    && tick < first_seen
                {
                    first_seen = tick;
                }
            }

            if first_seen == u64::MAX {
                first_seen = 0;
            }

            themes.push(BeliefTheme {
                keywords: cluster_keywords,
                first_seen_tick: first_seen,
                mention_count: total_mentions,
                adherent_ids: adherents,
            });
        }

        // Sort by adherent count descending
        themes.sort_by(|a, b| b.adherent_ids.len().cmp(&a.adherent_ids.len()));

        themes
    }

    /// Check for belief systems that meet the detection threshold.
    ///
    /// A belief system qualifies when:
    /// - 3+ agents share 2+ thematic keywords.
    ///
    /// Returns a list of detected beliefs.
    pub fn check_for_new_constructs(&self) -> Vec<DetectedBelief> {
        let mut results = Vec::new();

        let keywords: Vec<String> = self.keyword_agents.keys().cloned().collect();
        let keyword_count = keywords.len();

        // Track which agent-sets we have already reported to avoid duplicates.
        let mut seen_agent_sets: Vec<HashSet<AgentId>> = Vec::new();

        for i in 0..keyword_count {
            for j in (i.saturating_add(1))..keyword_count {
                let kw_i = keywords.get(i);
                let kw_j = keywords.get(j);

                if let (Some(ki), Some(kj)) = (kw_i, kw_j) {
                    let agents_i = self.keyword_agents.get(ki);
                    let agents_j = self.keyword_agents.get(kj);

                    if let (Some(ai), Some(aj)) = (agents_i, agents_j) {
                        let shared: HashSet<AgentId> =
                            ai.intersection(aj).copied().collect();

                        if shared.len() >= MIN_ADHERENTS_FOR_CONSTRUCT
                            && !seen_agent_sets.contains(&shared)
                        {
                            // Find all keywords shared by this agent set
                            let all_shared_keywords: Vec<String> = keywords
                                .iter()
                                .filter(|kw| {
                                    self.keyword_agents.get(*kw).is_some_and(|agents| {
                                        shared.iter().all(|a| agents.contains(a))
                                    })
                                })
                                .cloned()
                                .collect();

                            if all_shared_keywords.len() >= MIN_SHARED_THEMES_FOR_CONSTRUCT {
                                let first_seen = all_shared_keywords
                                    .iter()
                                    .filter_map(|kw| self.keyword_first_seen.get(kw))
                                    .copied()
                                    .min()
                                    .unwrap_or(0);

                                results.push(DetectedBelief {
                                    shared_keywords: all_shared_keywords,
                                    adherent_ids: shared.clone(),
                                    first_seen_tick: first_seen,
                                });

                                seen_agent_sets.push(shared);
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Get beliefs sorted by adherent count (descending).
    ///
    /// Returns detected clusters that meet the minimum threshold.
    pub fn get_dominant_beliefs(&self) -> Vec<BeliefTheme> {
        let mut clusters = self.detect_clusters();
        clusters.retain(|t| t.adherent_ids.len() >= MIN_ADHERENTS_FOR_CONSTRUCT);
        // Already sorted by adherent count from detect_clusters
        clusters
    }

    /// Detect schism risk within a belief system.
    ///
    /// A schism risk exists when a belief's adherents can be divided into
    /// two groups where each group has exclusive keywords not shared by
    /// the other group.
    ///
    /// `belief_agents` is the set of agents in the belief to analyze.
    /// Returns `Some(SchismRisk)` if divergence is detected.
    pub fn detect_schism_risk(
        &self,
        belief_agents: &HashSet<AgentId>,
    ) -> Option<SchismRisk> {
        if belief_agents.len() < 2 {
            return None;
        }

        // Collect all keywords used by agents in this belief
        let mut agent_keyword_sets: BTreeMap<AgentId, HashSet<String>> = BTreeMap::new();
        for agent in belief_agents {
            if let Some(kws) = self.agent_keywords.get(agent) {
                agent_keyword_sets.insert(*agent, kws.clone());
            }
        }

        if agent_keyword_sets.len() < 2 {
            return None;
        }

        // Compute the set of keywords shared by ALL agents
        let mut all_keywords_iter = agent_keyword_sets.values();
        let first = all_keywords_iter.next()?.clone();
        let shared_by_all = all_keywords_iter.fold(first, |acc, kws| {
            acc.intersection(kws).cloned().collect()
        });

        // For each agent, compute their "exclusive" keywords (not shared by all)
        let mut agent_exclusives: BTreeMap<AgentId, HashSet<String>> = BTreeMap::new();
        for (agent, kws) in &agent_keyword_sets {
            let exclusive: HashSet<String> = kws
                .difference(&shared_by_all)
                .cloned()
                .collect();
            if !exclusive.is_empty() {
                agent_exclusives.insert(*agent, exclusive);
            }
        }

        if agent_exclusives.len() < 2 {
            return None;
        }

        // Simple two-faction split: pick the first agent's exclusive set as primary,
        // group other agents into primary (overlapping exclusives) or divergent.
        let agents_with_exclusives: Vec<AgentId> = agent_exclusives.keys().copied().collect();

        let seed_agent = agents_with_exclusives.first()?;
        let seed_exclusive = agent_exclusives.get(seed_agent)?;

        let mut primary_agents = HashSet::new();
        let mut primary_kws: HashSet<String> = seed_exclusive.clone();

        let mut divergent_agents = HashSet::new();
        let mut divergent_kws: HashSet<String> = HashSet::new();

        primary_agents.insert(*seed_agent);

        for agent in agents_with_exclusives.iter().skip(1) {
            if let Some(exc) = agent_exclusives.get(agent) {
                if exc.intersection(seed_exclusive).count() > 0 {
                    primary_agents.insert(*agent);
                    for kw in exc {
                        primary_kws.insert(kw.clone());
                    }
                } else {
                    divergent_agents.insert(*agent);
                    for kw in exc {
                        divergent_kws.insert(kw.clone());
                    }
                }
            }
        }

        if divergent_agents.is_empty() || divergent_kws.is_empty() {
            return None;
        }

        Some(SchismRisk {
            primary_keywords: primary_kws.into_iter().collect(),
            primary_agents,
            divergent_keywords: divergent_kws.into_iter().collect(),
            divergent_agents,
        })
    }
}

impl Default for BeliefDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use emergence_types::AgentId;

    use super::*;

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn make_agents(count: usize) -> Vec<AgentId> {
        (0..count).map(|_| AgentId::new()).collect()
    }

    // -----------------------------------------------------------------------
    // 1. Basic keyword extraction
    // -----------------------------------------------------------------------

    #[test]
    fn record_communication_extracts_keywords() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 1, "I pray to the divine spirit");

        assert!(detector.keyword_agents.contains_key("pray"));
        assert!(detector.keyword_agents.contains_key("divine"));
        assert!(detector.keyword_agents.contains_key("spirit"));
        assert_eq!(detector.keyword_counts.get("pray").copied().unwrap_or(0), 1);
    }

    #[test]
    fn record_communication_case_insensitive() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 1, "The DIVINE TRUTH shall prevail");

        assert!(detector.keyword_agents.contains_key("divine"));
        assert!(detector.keyword_agents.contains_key("truth"));
    }

    #[test]
    fn record_communication_no_keywords() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 1, "Let us trade berries for wood");

        assert!(detector.keyword_agents.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. Mention counting
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_mentions_increment_count() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 1, "I pray daily");
        detector.record_communication(agent, 2, "I pray for wisdom");

        assert_eq!(detector.keyword_counts.get("pray").copied().unwrap_or(0), 2);
    }

    // -----------------------------------------------------------------------
    // 3. First seen tick
    // -----------------------------------------------------------------------

    #[test]
    fn first_seen_tick_recorded() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 5, "The soul is eternal");
        detector.record_communication(agent, 10, "The soul transcends");

        assert_eq!(
            detector.keyword_first_seen.get("soul").copied().unwrap_or(0),
            5
        );
    }

    // -----------------------------------------------------------------------
    // 4. Cluster detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_clusters_groups_shared_agent_keywords() {
        let mut detector = BeliefDetector::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        detector.record_communication(agent_a, 1, "I worship the divine");
        detector.record_communication(agent_b, 2, "divine worship is sacred");

        let clusters = detector.detect_clusters();
        assert!(!clusters.is_empty());

        let first = clusters.first();
        assert!(first.is_some_and(|c| c.adherent_ids.contains(&agent_a)));
        assert!(first.is_some_and(|c| c.adherent_ids.contains(&agent_b)));
    }

    #[test]
    fn detect_clusters_separates_unrelated() {
        let mut detector = BeliefDetector::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        detector.record_communication(agent_a, 1, "I pray to god");
        detector.record_communication(agent_b, 2, "the prophecy of destiny");

        let clusters = detector.detect_clusters();
        assert_eq!(clusters.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 5. New construct detection
    // -----------------------------------------------------------------------

    #[test]
    fn check_for_new_constructs_meets_threshold() {
        let mut detector = BeliefDetector::new();
        let agents = make_agents(4);

        for (i, agent) in agents.iter().enumerate() {
            let tick = u64::try_from(i).unwrap_or(0);
            detector.record_communication(
                *agent,
                tick,
                "The divine sacred truth guides us",
            );
        }

        let constructs = detector.check_for_new_constructs();
        assert!(!constructs.is_empty());

        let first = constructs.first();
        assert!(first.is_some_and(|c| c.adherent_ids.len() >= 3));
        assert!(first.is_some_and(|c| c.shared_keywords.len() >= 2));
    }

    #[test]
    fn check_for_new_constructs_below_threshold() {
        let mut detector = BeliefDetector::new();
        let agents = make_agents(2);

        for (i, agent) in agents.iter().enumerate() {
            let tick = u64::try_from(i).unwrap_or(0);
            detector.record_communication(*agent, tick, "divine sacred");
        }

        let constructs = detector.check_for_new_constructs();
        assert!(constructs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 6. Dominant beliefs
    // -----------------------------------------------------------------------

    #[test]
    fn get_dominant_beliefs_sorted_by_adherents() {
        let mut detector = BeliefDetector::new();

        let large_group = make_agents(5);
        for (i, agent) in large_group.iter().enumerate() {
            let tick = u64::try_from(i).unwrap_or(0);
            detector.record_communication(*agent, tick, "The divine sacred holy worship");
        }

        let dominant = detector.get_dominant_beliefs();
        if !dominant.is_empty() {
            let first = dominant.first();
            assert!(first.is_some_and(|b| b.adherent_ids.len() >= 3));
        }
    }

    // -----------------------------------------------------------------------
    // 7. Schism detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_schism_risk_with_diverging_factions() {
        let mut detector = BeliefDetector::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let agent_c = AgentId::new();

        detector.record_communication(agent_a, 1, "divine sacred worship");
        detector.record_communication(agent_b, 2, "divine sacred pray");
        detector.record_communication(agent_c, 3, "divine sacred freedom justice");

        let mut belief_agents = HashSet::new();
        belief_agents.insert(agent_a);
        belief_agents.insert(agent_b);
        belief_agents.insert(agent_c);

        let risk = detector.detect_schism_risk(&belief_agents);
        assert!(risk.is_some());
    }

    #[test]
    fn detect_schism_risk_no_divergence() {
        let mut detector = BeliefDetector::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        detector.record_communication(agent_a, 1, "divine sacred holy");
        detector.record_communication(agent_b, 2, "divine sacred holy");

        let mut belief_agents = HashSet::new();
        belief_agents.insert(agent_a);
        belief_agents.insert(agent_b);

        let risk = detector.detect_schism_risk(&belief_agents);
        assert!(risk.is_none());
    }

    #[test]
    fn detect_schism_risk_single_agent_returns_none() {
        let detector = BeliefDetector::new();
        let agent = AgentId::new();

        let mut agents = HashSet::new();
        agents.insert(agent);

        let risk = detector.detect_schism_risk(&agents);
        assert!(risk.is_none());
    }

    // -----------------------------------------------------------------------
    // 8. Empty detector
    // -----------------------------------------------------------------------

    #[test]
    fn empty_detector_returns_no_clusters() {
        let detector = BeliefDetector::new();
        assert!(detector.detect_clusters().is_empty());
        assert!(detector.check_for_new_constructs().is_empty());
        assert!(detector.get_dominant_beliefs().is_empty());
    }

    // -----------------------------------------------------------------------
    // 9. Multiple keyword categories
    // -----------------------------------------------------------------------

    #[test]
    fn keywords_span_multiple_categories() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(
            agent,
            1,
            "The divine prophecy speaks of justice and freedom",
        );

        let agent_kws = detector.agent_keywords.get(&agent);
        assert!(agent_kws.is_some());
        let kws = agent_kws.unwrap_or(&HashSet::new()).clone();
        assert!(kws.contains("divine"));
        assert!(kws.contains("prophecy"));
        assert!(kws.contains("justice"));
        assert!(kws.contains("freedom"));
    }

    // -----------------------------------------------------------------------
    // 10. Adherent tracking across messages
    // -----------------------------------------------------------------------

    #[test]
    fn agent_tracked_across_multiple_messages() {
        let mut detector = BeliefDetector::new();
        let agent = AgentId::new();

        detector.record_communication(agent, 1, "I pray to god");
        detector.record_communication(agent, 5, "The soul is divine");

        let agent_kws = detector.agent_keywords.get(&agent);
        assert!(agent_kws.is_some());
        let kws = agent_kws.unwrap_or(&HashSet::new()).clone();
        assert!(kws.contains("pray"));
        assert!(kws.contains("god"));
        assert!(kws.contains("soul"));
        assert!(kws.contains("divine"));
    }

    // -----------------------------------------------------------------------
    // 11. Construct detection with exact threshold
    // -----------------------------------------------------------------------

    #[test]
    fn exactly_three_agents_two_keywords_detected() {
        let mut detector = BeliefDetector::new();
        let agents = make_agents(3);

        for (i, agent) in agents.iter().enumerate() {
            let tick = u64::try_from(i).unwrap_or(0);
            detector.record_communication(*agent, tick, "divine sacred");
        }

        let constructs = detector.check_for_new_constructs();
        assert!(!constructs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 12. Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn default_detector_is_empty() {
        let detector = BeliefDetector::default();
        assert!(detector.keyword_agents.is_empty());
        assert!(detector.agent_keywords.is_empty());
    }
}
