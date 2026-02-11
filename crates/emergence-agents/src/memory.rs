//! Agent memory system: tiered storage, compression, and perception filtering.
//!
//! Implements the memory system from `agent-system.md` section 4:
//!
//! - **Immediate**: Last N ticks (default 5), full detail of all perceived events
//! - **Short-term**: Last N ticks (default 50), summarized events and key interactions
//! - **Long-term**: Lifetime, major milestones, relationship formation, discoveries, deaths
//!
//! Memory compression runs at the end of each tick (Reflection phase) and
//! promotes, summarizes, or discards memories based on their `emotional_weight`.
//!
//! Memory filtering assembles the subset of memories relevant to the current
//! perception payload, respecting a configurable token budget.
//!
//! ## Importance scoring
//!
//! [`importance_score`] evaluates a memory entry's content to produce a numeric
//! importance rating. Social events and combat score highest (3.0), discoveries
//! score medium-high (2.5), and routine activities score lowest (1.0). The score
//! is computed on-demand from the memory summary text.
//!
//! ## Reflection triggers
//!
//! [`find_reflection_triggers`] scans an agent's memories for entries that are
//! contextually relevant to the current perception -- matching the agent's
//! current location name or the names of visible agents. High-importance matches
//! are returned as "reflection triggers" to be injected into the LLM prompt so
//! the agent can recall significant past events.
//!
//! ## Compression logging
//!
//! [`CompressionRecord`] captures metadata about each compression pass: how many
//! memories were present, how many were dropped, the importance scores of dropped
//! entries, and a human-readable summary. The record is logged via `tracing` for
//! post-hoc analysis.

use rust_decimal::Decimal;
use uuid::Uuid;

use emergence_types::{MemoryEntry, MemoryTier};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the agent memory system.
///
/// Controls retention periods for each tier and the maximum token budget
/// for memories included in perception payloads.
///
/// See `agent-system.md` section 12 for default values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    /// Number of ticks that immediate memories are retained (default: 5).
    pub immediate_retention_ticks: u64,

    /// Number of ticks that short-term memories are retained (default: 50).
    pub short_term_retention_ticks: u64,

    /// Maximum approximate token count for memories in a perception payload
    /// (default: 2000). Uses character count / 4 as a rough token estimate.
    pub max_memory_tokens: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            immediate_retention_ticks: 5,
            short_term_retention_ticks: 50,
            max_memory_tokens: 2000,
        }
    }
}

// ---------------------------------------------------------------------------
// Emotional weight thresholds (as Decimal constants)
// ---------------------------------------------------------------------------

/// Threshold above which a memory is promoted to long-term (> 0.7).
fn high_weight_threshold() -> Decimal {
    Decimal::new(7, 1)
}

/// Threshold below which a memory is discarded (< 0.3).
fn low_weight_threshold() -> Decimal {
    Decimal::new(3, 1)
}

// ---------------------------------------------------------------------------
// Importance scoring (Phase 8.4.2)
// ---------------------------------------------------------------------------

// Keyword lists for importance scoring categories.
//
// Each list maps a set of keywords to an importance level. The keywords are
// checked as case-insensitive substrings of the memory summary.

/// Social event keywords -- score 3.0.
const SOCIAL_KEYWORDS: &[&str] = &[
    "betray", "alliance", "trade", "dispute", "leader", "war", "marriage",
    "death", "born", "treaty", "divorce", "conspire", "vote", "propose",
];

/// Discovery and learning keywords -- score 2.5.
const DISCOVERY_KEYWORDS: &[&str] = &[
    "discover", "learn", "invent", "knowledge", "teach", "research", "craft",
];

/// Combat and conflict keywords -- score 3.0.
const COMBAT_KEYWORDS: &[&str] = &[
    "attack", "defend", "steal", "combat", "threat", "intimidate", "wound",
    "kill", "fight", "raid",
];

/// Routine activity keywords -- score 1.0.
const ROUTINE_KEYWORDS: &[&str] = &[
    "gather", "eat", "drink", "rest", "sleep", "travel", "move", "walk",
    "idle", "wait",
];

/// Importance score for social events.
const IMPORTANCE_SOCIAL: f64 = 3.0;

/// Importance score for discoveries.
const IMPORTANCE_DISCOVERY: f64 = 2.5;

/// Importance score for combat and conflict.
const IMPORTANCE_COMBAT: f64 = 3.0;

/// Importance score for routine activities.
const IMPORTANCE_ROUTINE: f64 = 1.0;

/// Default importance score when no keywords match.
const IMPORTANCE_DEFAULT: f64 = 1.5;

/// Evaluate the importance of a memory entry based on its summary content.
///
/// Returns a floating-point score:
/// - 3.0 for social events (betray, alliance, trade, war, etc.)
/// - 3.0 for combat/conflict (attack, defend, steal, etc.)
/// - 2.5 for discoveries (discover, learn, invent, etc.)
/// - 1.0 for routine activities (gather, eat, rest, travel, etc.)
/// - 1.5 default for anything else
///
/// If multiple categories match, the highest score wins.
pub fn importance_score(entry: &MemoryEntry) -> f64 {
    let summary_lower = entry.summary.to_lowercase();

    let mut best = IMPORTANCE_DEFAULT;

    // Check highest-value categories first.
    if matches_any_keyword(&summary_lower, SOCIAL_KEYWORDS) && best < IMPORTANCE_SOCIAL {
        best = IMPORTANCE_SOCIAL;
    }
    if matches_any_keyword(&summary_lower, COMBAT_KEYWORDS) && best < IMPORTANCE_COMBAT {
        best = IMPORTANCE_COMBAT;
    }
    if matches_any_keyword(&summary_lower, DISCOVERY_KEYWORDS) && best < IMPORTANCE_DISCOVERY {
        best = IMPORTANCE_DISCOVERY;
    }
    // Routine can only lower the score from default, so only apply if nothing
    // else matched (i.e. best is still at default).
    if best <= IMPORTANCE_DEFAULT
        && matches_any_keyword(&summary_lower, ROUTINE_KEYWORDS)
    {
        best = IMPORTANCE_ROUTINE;
    }

    best
}

/// Check whether `text` contains any of the given keywords as substrings.
fn matches_any_keyword(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

// ---------------------------------------------------------------------------
// Reflection triggers (Phase 8.4.1)
// ---------------------------------------------------------------------------

/// Minimum importance score for a memory to qualify as a reflection trigger.
const REFLECTION_IMPORTANCE_THRESHOLD: f64 = 2.5;

/// Find memories that should trigger agent reflection given the current context.
///
/// Scans `memories` for entries that are both high-importance (score >= 2.5)
/// and contextually relevant to the agent's current perception:
///
/// - The memory summary mentions the `current_location` name (case-insensitive)
/// - The memory summary mentions any of the `visible_agents` names (case-insensitive)
///
/// Results are sorted by importance score descending, then by tick descending
/// (most recent first). At most `max_results` entries are returned.
///
/// These memories should be injected into the LLM prompt as "reflection triggers"
/// so the agent can recall past significant events relevant to the current moment.
pub fn find_reflection_triggers<'a>(
    memories: &'a [MemoryEntry],
    current_location: &str,
    visible_agents: &[String],
    max_results: usize,
) -> Vec<&'a MemoryEntry> {
    let location_lower = current_location.to_lowercase();
    let agent_names_lower: Vec<String> = visible_agents
        .iter()
        .map(|n| n.to_lowercase())
        .collect();

    let mut candidates: Vec<(f64, &'a MemoryEntry)> = memories
        .iter()
        .filter_map(|entry| {
            let score = importance_score(entry);
            if score < REFLECTION_IMPORTANCE_THRESHOLD {
                return None;
            }

            let summary_lower = entry.summary.to_lowercase();

            // Check if the memory is contextually relevant.
            let location_match = !location_lower.is_empty()
                && summary_lower.contains(&location_lower);
            let agent_match = agent_names_lower
                .iter()
                .any(|name| !name.is_empty() && summary_lower.contains(name.as_str()));

            if location_match || agent_match {
                Some((score, entry))
            } else {
                None
            }
        })
        .collect();

    // Sort by importance descending, then by tick descending (most recent first).
    candidates.sort_by(|a, b| {
        b.0.total_cmp(&a.0).then_with(|| b.1.tick.cmp(&a.1.tick))
    });

    candidates
        .into_iter()
        .take(max_results)
        .map(|(_, entry)| entry)
        .collect()
}

// ---------------------------------------------------------------------------
// Compression logging (Phase 8.4.3)
// ---------------------------------------------------------------------------

/// Record of a single memory compression pass.
///
/// Created by [`MemoryStore::compress`] whenever memories are promoted or
/// discarded. Contains enough information to understand what was lost and why.
#[derive(Debug, Clone, PartialEq)]
pub struct CompressionRecord {
    /// The tick at which compression was performed.
    pub tick: u64,
    /// Number of memory entries before compression.
    pub original_count: usize,
    /// Number of memory entries after compression.
    pub compressed_count: usize,
    /// Importance scores of the entries that were discarded (dropped).
    pub importance_scores_dropped: Vec<f64>,
    /// Brief human-readable description of what happened.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// MemoryStore
// ---------------------------------------------------------------------------

/// Manages all three tiers of agent memory.
///
/// The store holds a flat list of [`MemoryEntry`] values, each tagged with its
/// tier. The store provides methods to add new memories, compress old ones
/// (promoting, summarizing, or discarding based on emotional weight), and
/// filter relevant memories for inclusion in perception payloads.
///
/// # Lifecycle
///
/// 1. New memories are added via [`add`](Self::add) as [`MemoryTier::Immediate`].
/// 2. At the end of each tick, [`compress`](Self::compress) is called.
/// 3. Before building a perception payload, [`relevant_memories`](Self::relevant_memories)
///    returns the subset the agent should "remember" this tick.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    /// All memories across all tiers, ordered by insertion.
    entries: Vec<MemoryEntry>,

    /// Configuration controlling retention and token budgets.
    config: MemoryConfig,
}

impl MemoryStore {
    /// Create a new empty memory store with the given configuration.
    pub const fn new(config: MemoryConfig) -> Self {
        Self {
            entries: Vec::new(),
            config,
        }
    }

    /// Create a memory store pre-populated with existing memories.
    ///
    /// Used when restoring agent state from persistence (Dragonfly or Postgres).
    pub const fn with_entries(config: MemoryConfig, entries: Vec<MemoryEntry>) -> Self {
        Self { entries, config }
    }

    /// Add a new memory entry (always starts as [`MemoryTier::Immediate`]).
    pub fn add(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
    }

    /// Return a reference to all stored memories.
    pub fn entries(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Return the current configuration.
    pub const fn config(&self) -> &MemoryConfig {
        &self.config
    }

    /// Return the total number of memories across all tiers.
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store contains no memories.
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Count memories in a specific tier.
    pub fn count_tier(&self, tier: MemoryTier) -> usize {
        self.entries.iter().filter(|e| e.tier == tier).count()
    }

    /// Export all entries as a `Vec` (e.g. for serializing to `AgentState.memory`).
    pub fn to_vec(&self) -> Vec<MemoryEntry> {
        self.entries.clone()
    }

    // -----------------------------------------------------------------------
    // Compression (agent-system.md section 4.3)
    // -----------------------------------------------------------------------

    /// Compress memories based on age and emotional weight.
    ///
    /// Called at the end of each tick during the Reflection phase.
    ///
    /// **Immediate memories** older than `immediate_retention_ticks`:
    /// - `emotional_weight` > 0.7 --> promote to [`MemoryTier::LongTerm`]
    /// - `emotional_weight` in 0.3..=0.7 --> promote to [`MemoryTier::ShortTerm`]
    /// - `emotional_weight` < 0.3 --> discard
    ///
    /// **Short-term memories** older than `short_term_retention_ticks`:
    /// - `emotional_weight` > 0.7 --> promote to [`MemoryTier::LongTerm`]
    /// - Otherwise --> discard
    ///
    /// Returns a [`CompressionRecord`] describing what happened during the pass.
    /// The record is also logged via `tracing::debug!` for observability.
    pub fn compress(&mut self, current_tick: u64) -> CompressionRecord {
        let original_count = self.entries.len();
        let immediate_cutoff = current_tick.saturating_sub(self.config.immediate_retention_ticks);
        let short_term_cutoff = current_tick.saturating_sub(self.config.short_term_retention_ticks);

        let high = high_weight_threshold();
        let low = low_weight_threshold();

        // Process each entry in place: update tier or mark for removal.
        // We iterate once and collect indices to remove (in reverse order).
        let mut to_remove: Vec<usize> = Vec::new();
        let mut promoted_to_long_term: usize = 0;
        let mut promoted_to_short_term: usize = 0;

        let entry_count = self.entries.len();
        for i in 0..entry_count {
            // Safe: i is always < entry_count which equals self.entries.len()
            let Some(entry) = self.entries.get_mut(i) else {
                continue;
            };

            match entry.tier {
                MemoryTier::Immediate => {
                    // Only compress if older than the immediate retention window
                    if entry.tick < immediate_cutoff {
                        if entry.emotional_weight > high {
                            entry.tier = MemoryTier::LongTerm;
                            promoted_to_long_term = promoted_to_long_term.saturating_add(1);
                        } else if entry.emotional_weight >= low {
                            entry.tier = MemoryTier::ShortTerm;
                            promoted_to_short_term = promoted_to_short_term.saturating_add(1);
                        } else {
                            to_remove.push(i);
                        }
                    }
                }
                MemoryTier::ShortTerm => {
                    // Only compress if older than the short-term retention window
                    if entry.tick < short_term_cutoff {
                        if entry.emotional_weight > high {
                            entry.tier = MemoryTier::LongTerm;
                            promoted_to_long_term = promoted_to_long_term.saturating_add(1);
                        } else {
                            to_remove.push(i);
                        }
                    }
                }
                MemoryTier::LongTerm => {
                    // Long-term memories are never discarded by compression
                }
            }
        }

        // Compute importance scores for entries about to be discarded.
        let importance_scores_dropped: Vec<f64> = to_remove
            .iter()
            .filter_map(|&idx| self.entries.get(idx))
            .map(importance_score)
            .collect();

        let dropped_count = to_remove.len();

        // Remove discarded entries in reverse index order to preserve indices.
        for &idx in to_remove.iter().rev() {
            self.entries.swap_remove(idx);
        }

        let compressed_count = self.entries.len();

        // Build a human-readable summary.
        let summary = format!(
            "tick {current_tick}: {original_count} -> {compressed_count} entries \
             ({dropped_count} dropped, {promoted_to_long_term} promoted to long-term, \
             {promoted_to_short_term} promoted to short-term)"
        );

        let record = CompressionRecord {
            tick: current_tick,
            original_count,
            compressed_count,
            importance_scores_dropped,
            summary: summary.clone(),
        };

        if dropped_count > 0 || promoted_to_long_term > 0 || promoted_to_short_term > 0 {
            tracing::debug!(
                tick = current_tick,
                original_count,
                compressed_count,
                dropped_count,
                promoted_to_long_term,
                promoted_to_short_term,
                "memory compression: {summary}"
            );
        }

        record
    }

    // -----------------------------------------------------------------------
    // Filtering for perception (agent-system.md section 4.4)
    // -----------------------------------------------------------------------

    /// Return memories relevant to the current perception context.
    ///
    /// Includes:
    /// - **All** immediate memories (last N ticks, full detail)
    /// - **Relevant** short-term memories (those referencing the current location,
    ///   nearby agents, or matching active goal keywords)
    /// - **All** long-term memories (always included, typically small set)
    ///
    /// Results are capped at [`MemoryConfig::max_memory_tokens`] approximate
    /// tokens. Long-term memories are included first (highest priority), then
    /// immediate, then filtered short-term. If the budget is exhausted,
    /// remaining memories are dropped.
    pub fn relevant_memories(
        &self,
        current_tick: u64,
        location_id: Uuid,
        nearby_agents: &[Uuid],
        goals: &[String],
    ) -> Vec<&MemoryEntry> {
        let immediate_cutoff = current_tick.saturating_sub(self.config.immediate_retention_ticks);

        // Categorize entries
        let mut long_term: Vec<&MemoryEntry> = Vec::new();
        let mut immediate: Vec<&MemoryEntry> = Vec::new();
        let mut short_term_relevant: Vec<&MemoryEntry> = Vec::new();

        for entry in &self.entries {
            match entry.tier {
                MemoryTier::LongTerm => {
                    long_term.push(entry);
                }
                MemoryTier::Immediate => {
                    // Only include if within the immediate window
                    if entry.tick >= immediate_cutoff {
                        immediate.push(entry);
                    }
                }
                MemoryTier::ShortTerm => {
                    // Include if relevant to current context
                    if is_relevant(entry, location_id, nearby_agents, goals) {
                        short_term_relevant.push(entry);
                    }
                }
            }
        }

        // Sort each group by tick (most recent first within category)
        // Using reverse sort so newer memories come first
        long_term.sort_by(|a, b| b.tick.cmp(&a.tick));
        immediate.sort_by(|a, b| b.tick.cmp(&a.tick));
        short_term_relevant.sort_by(|a, b| b.tick.cmp(&a.tick));

        // Assemble results respecting token budget.
        // Priority: long-term first, then immediate, then short-term.
        let mut result: Vec<&MemoryEntry> = Vec::new();
        let mut token_budget = self.config.max_memory_tokens;

        // Phase 1: long-term (always included if budget allows)
        for entry in &long_term {
            let tokens = entry.approximate_tokens();
            if tokens <= token_budget {
                result.push(entry);
                // Safe: tokens <= token_budget, so subtraction cannot underflow
                token_budget = token_budget.saturating_sub(tokens);
            } else {
                break;
            }
        }

        // Phase 2: immediate (all recent memories if budget allows)
        for entry in &immediate {
            let tokens = entry.approximate_tokens();
            if tokens <= token_budget {
                result.push(entry);
                token_budget = token_budget.saturating_sub(tokens);
            } else {
                break;
            }
        }

        // Phase 3: relevant short-term (fill remaining budget)
        for entry in &short_term_relevant {
            let tokens = entry.approximate_tokens();
            if tokens <= token_budget {
                result.push(entry);
                token_budget = token_budget.saturating_sub(tokens);
            } else {
                break;
            }
        }

        result
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new(MemoryConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Relevance check helper
// ---------------------------------------------------------------------------

/// Check whether a short-term memory is relevant to the current context.
///
/// A memory is relevant if it:
/// - References the agent's current location
/// - References any nearby agent
/// - Matches any word from an active goal (case-insensitive substring)
///
/// Goal strings are split into individual words so that a goal like
/// "find food" will match a memory whose summary contains "food".
fn is_relevant(
    entry: &MemoryEntry,
    location_id: Uuid,
    nearby_agents: &[Uuid],
    goals: &[String],
) -> bool {
    // Check location relevance
    if entry.involves_entity(location_id) {
        return true;
    }

    // Check nearby agent relevance
    if entry.involves_any_entity(nearby_agents) {
        return true;
    }

    // Check goal keyword relevance: split each goal into words and check each.
    // Words shorter than 3 characters are skipped to avoid false positives
    // from articles and prepositions ("a", "to", "of", etc.).
    for goal in goals {
        for word in goal.split_whitespace() {
            if word.len() >= 3 && entry.matches_topic(word) {
                return true;
            }
        }
    }

    false
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use emergence_types::{MemoryEntry, MemoryTier};

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_memory(tick: u64, weight: Decimal, tier: MemoryTier) -> MemoryEntry {
        MemoryEntry {
            tick,
            memory_type: String::from("observation"),
            summary: String::from("Something happened nearby"),
            entities: Vec::new(),
            emotional_weight: weight,
            tier,
        }
    }

    fn make_memory_with_entities(
        tick: u64,
        weight: Decimal,
        tier: MemoryTier,
        entities: Vec<Uuid>,
    ) -> MemoryEntry {
        MemoryEntry {
            tick,
            memory_type: String::from("observation"),
            summary: String::from("Observed agents trading"),
            entities,
            emotional_weight: weight,
            tier,
        }
    }

    fn make_memory_with_summary(
        tick: u64,
        weight: Decimal,
        tier: MemoryTier,
        summary: &str,
    ) -> MemoryEntry {
        MemoryEntry {
            tick,
            memory_type: String::from("action"),
            summary: String::from(summary),
            entities: Vec::new(),
            emotional_weight: weight,
            tier,
        }
    }

    // -----------------------------------------------------------------------
    // MemoryConfig tests
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_values() {
        let config = MemoryConfig::default();
        assert_eq!(config.immediate_retention_ticks, 5);
        assert_eq!(config.short_term_retention_ticks, 50);
        assert_eq!(config.max_memory_tokens, 2000);
    }

    // -----------------------------------------------------------------------
    // MemoryStore basic operations
    // -----------------------------------------------------------------------

    #[test]
    fn new_store_is_empty() {
        let store = MemoryStore::default();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn add_memory_increases_count() {
        let mut store = MemoryStore::default();
        store.add(make_memory(1, Decimal::new(5, 1), MemoryTier::Immediate));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn count_tier_correct() {
        let mut store = MemoryStore::default();
        store.add(make_memory(1, Decimal::new(5, 1), MemoryTier::Immediate));
        store.add(make_memory(2, Decimal::new(5, 1), MemoryTier::Immediate));
        store.add(make_memory(3, Decimal::new(8, 1), MemoryTier::LongTerm));

        assert_eq!(store.count_tier(MemoryTier::Immediate), 2);
        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 0);
        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
    }

    #[test]
    fn with_entries_restores_state() {
        let entries = vec![
            make_memory(1, Decimal::new(5, 1), MemoryTier::Immediate),
            make_memory(10, Decimal::new(8, 1), MemoryTier::LongTerm),
        ];
        let store = MemoryStore::with_entries(MemoryConfig::default(), entries);
        assert_eq!(store.len(), 2);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 1);
        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
    }

    #[test]
    fn to_vec_returns_all_entries() {
        let mut store = MemoryStore::default();
        store.add(make_memory(1, Decimal::new(5, 1), MemoryTier::Immediate));
        store.add(make_memory(2, Decimal::new(8, 1), MemoryTier::LongTerm));
        let exported = store.to_vec();
        assert_eq!(exported.len(), 2);
    }

    // -----------------------------------------------------------------------
    // MemoryEntry constructors
    // -----------------------------------------------------------------------

    #[test]
    fn action_constructor_sets_fields() {
        let entry = MemoryEntry::action(
            10,
            String::from("Gathered wood"),
            vec![Uuid::nil()],
            Decimal::new(5, 1),
        );
        assert_eq!(entry.tick, 10);
        assert_eq!(entry.memory_type, "action");
        assert_eq!(entry.tier, MemoryTier::Immediate);
        assert_eq!(entry.emotional_weight, Decimal::new(5, 1));
    }

    #[test]
    fn observation_constructor_sets_type() {
        let entry = MemoryEntry::observation(
            5,
            String::from("Saw a bear"),
            Vec::new(),
            Decimal::new(9, 1),
        );
        assert_eq!(entry.memory_type, "observation");
        assert_eq!(entry.tier, MemoryTier::Immediate);
    }

    #[test]
    fn communication_constructor_sets_type() {
        let entry = MemoryEntry::communication(
            5,
            String::from("Talked to Kora"),
            Vec::new(),
            Decimal::new(4, 1),
        );
        assert_eq!(entry.memory_type, "communication");
    }

    #[test]
    fn discovery_constructor_sets_type() {
        let entry = MemoryEntry::discovery(
            5,
            String::from("Discovered fire"),
            Vec::new(),
            Decimal::new(9, 1),
        );
        assert_eq!(entry.memory_type, "discovery");
    }

    #[test]
    fn social_constructor_sets_type() {
        let entry = MemoryEntry::social(
            5,
            String::from("Formed alliance"),
            Vec::new(),
            Decimal::new(8, 1),
        );
        assert_eq!(entry.memory_type, "social");
    }

    #[test]
    fn constructor_clamps_weight_above_one() {
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            Vec::new(),
            Decimal::new(15, 1), // 1.5, should clamp to 1.0
        );
        assert_eq!(entry.emotional_weight, Decimal::ONE);
    }

    #[test]
    fn constructor_clamps_weight_below_zero() {
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            Vec::new(),
            Decimal::new(-5, 1), // -0.5, should clamp to 0.0
        );
        assert_eq!(entry.emotional_weight, Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // MemoryEntry relevance methods
    // -----------------------------------------------------------------------

    #[test]
    fn involves_entity_found() {
        let id = Uuid::new_v4();
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            vec![id],
            Decimal::new(5, 1),
        );
        assert!(entry.involves_entity(id));
    }

    #[test]
    fn involves_entity_not_found() {
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            vec![Uuid::new_v4()],
            Decimal::new(5, 1),
        );
        assert!(!entry.involves_entity(Uuid::new_v4()));
    }

    #[test]
    fn involves_any_entity_partial_match() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            vec![id1, id2],
            Decimal::new(5, 1),
        );
        assert!(entry.involves_any_entity(&[id3, id2]));
    }

    #[test]
    fn involves_any_entity_no_match() {
        let entry = MemoryEntry::action(
            1,
            String::from("test"),
            vec![Uuid::new_v4()],
            Decimal::new(5, 1),
        );
        assert!(!entry.involves_any_entity(&[Uuid::new_v4(), Uuid::new_v4()]));
    }

    #[test]
    fn matches_topic_case_insensitive() {
        let entry = MemoryEntry::action(
            1,
            String::from("Gathered WOOD from the forest"),
            Vec::new(),
            Decimal::new(5, 1),
        );
        assert!(entry.matches_topic("wood"));
        assert!(entry.matches_topic("WOOD"));
        assert!(entry.matches_topic("Forest"));
    }

    #[test]
    fn matches_topic_not_found() {
        let entry = MemoryEntry::action(
            1,
            String::from("Gathered wood"),
            Vec::new(),
            Decimal::new(5, 1),
        );
        assert!(!entry.matches_topic("stone"));
    }

    #[test]
    fn approximate_tokens_empty_summary() {
        let entry = MemoryEntry::action(
            1,
            String::new(),
            Vec::new(),
            Decimal::new(5, 1),
        );
        assert_eq!(entry.approximate_tokens(), 0);
    }

    #[test]
    fn approximate_tokens_short_summary() {
        // "hi" = 2 chars -> 2/4 = 0 -> minimum 1
        let entry = MemoryEntry::action(
            1,
            String::from("hi"),
            Vec::new(),
            Decimal::new(5, 1),
        );
        assert_eq!(entry.approximate_tokens(), 1);
    }

    #[test]
    fn approximate_tokens_normal_summary() {
        // "Gathered wood from the forest" = 29 chars -> 29/4 = 7
        let entry = MemoryEntry::action(
            1,
            String::from("Gathered wood from the forest"),
            Vec::new(),
            Decimal::new(5, 1),
        );
        assert_eq!(entry.approximate_tokens(), 7);
    }

    // -----------------------------------------------------------------------
    // Compression: immediate -> promote/discard
    // -----------------------------------------------------------------------

    #[test]
    fn compress_immediate_high_weight_promotes_to_long_term() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.8 (> 0.7)
        store.add(make_memory(0, Decimal::new(8, 1), MemoryTier::Immediate));

        // Current tick = 10 (so tick 0 is older than 5-tick retention)
        store.compress(10);

        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 0);
    }

    #[test]
    fn compress_immediate_medium_weight_promotes_to_short_term() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.5 (between 0.3 and 0.7)
        store.add(make_memory(0, Decimal::new(5, 1), MemoryTier::Immediate));

        store.compress(10);

        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 0);
    }

    #[test]
    fn compress_immediate_low_weight_discards() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.1 (< 0.3)
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::Immediate));

        store.compress(10);

        assert!(store.is_empty());
    }

    #[test]
    fn compress_immediate_at_boundary_0_3_promotes_to_short_term() {
        let mut store = MemoryStore::default();
        // Weight exactly 0.3 -> >= low threshold -> short-term
        store.add(make_memory(0, Decimal::new(3, 1), MemoryTier::Immediate));

        store.compress(10);

        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);
    }

    #[test]
    fn compress_immediate_at_boundary_0_7_promotes_to_short_term() {
        let mut store = MemoryStore::default();
        // Weight exactly 0.7 -> not > 0.7, but >= 0.3 -> short-term
        store.add(make_memory(0, Decimal::new(7, 1), MemoryTier::Immediate));

        store.compress(10);

        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);
    }

    #[test]
    fn compress_does_not_touch_recent_immediate_memories() {
        let mut store = MemoryStore::default();
        // Tick 8 with current_tick=10, retention=5 -> cutoff=5
        // Tick 8 >= 5, so it stays as immediate
        store.add(make_memory(8, Decimal::new(1, 1), MemoryTier::Immediate));

        store.compress(10);

        assert_eq!(store.count_tier(MemoryTier::Immediate), 1);
    }

    // -----------------------------------------------------------------------
    // Compression: short-term -> promote/discard
    // -----------------------------------------------------------------------

    #[test]
    fn compress_short_term_high_weight_promotes_to_long_term() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.8, already short-term
        store.add(make_memory(0, Decimal::new(8, 1), MemoryTier::ShortTerm));

        // Current tick = 60 (so tick 0 is older than 50-tick retention)
        store.compress(60);

        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 0);
    }

    #[test]
    fn compress_short_term_low_weight_discards() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.2, already short-term
        store.add(make_memory(0, Decimal::new(2, 1), MemoryTier::ShortTerm));

        store.compress(60);

        assert!(store.is_empty());
    }

    #[test]
    fn compress_short_term_medium_weight_discards() {
        let mut store = MemoryStore::default();
        // Tick 0, weight 0.5 -> not > 0.7 -> discard for expired short-term
        store.add(make_memory(0, Decimal::new(5, 1), MemoryTier::ShortTerm));

        store.compress(60);

        assert!(store.is_empty());
    }

    #[test]
    fn compress_does_not_touch_recent_short_term_memories() {
        let mut store = MemoryStore::default();
        // Tick 20 with current_tick=60, retention=50 -> cutoff=10
        // Tick 20 >= 10, so it stays as short-term
        store.add(make_memory(20, Decimal::new(2, 1), MemoryTier::ShortTerm));

        store.compress(60);

        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);
    }

    #[test]
    fn compress_never_discards_long_term() {
        let mut store = MemoryStore::default();
        // Even low-weight long-term memories survive
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::LongTerm));

        store.compress(1000);

        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
    }

    // -----------------------------------------------------------------------
    // Compression: multi-tick simulation
    // -----------------------------------------------------------------------

    #[test]
    fn compress_multi_tick_lifecycle() {
        let mut store = MemoryStore::default();

        // Tick 0: add a high-weight memory
        store.add(make_memory(0, Decimal::new(9, 1), MemoryTier::Immediate));
        // Tick 0: add a low-weight memory
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::Immediate));
        // Tick 0: add a medium-weight memory
        store.add(make_memory(0, Decimal::new(5, 1), MemoryTier::Immediate));

        // Compress at tick 10 (all three are older than 5 ticks)
        store.compress(10);

        // High -> long-term, medium -> short-term, low -> discarded
        assert_eq!(store.len(), 2);
        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);

        // Now compress at tick 60 (short-term is now older than 50 ticks)
        // The short-term memory has weight 0.5, not > 0.7, so it gets discarded
        store.compress(60);

        assert_eq!(store.len(), 1);
        assert_eq!(store.count_tier(MemoryTier::LongTerm), 1);
    }

    // -----------------------------------------------------------------------
    // Compression: edge case at tick 0
    // -----------------------------------------------------------------------

    #[test]
    fn compress_at_tick_zero_no_crash() {
        let mut store = MemoryStore::default();
        store.add(make_memory(0, Decimal::new(5, 1), MemoryTier::Immediate));
        // current_tick = 0, cutoff = 0.saturating_sub(5) = 0
        // tick 0 is NOT < 0, so nothing happens
        store.compress(0);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 1);
    }

    // -----------------------------------------------------------------------
    // Filtering: relevant_memories
    // -----------------------------------------------------------------------

    #[test]
    fn relevant_memories_includes_all_long_term() {
        let mut store = MemoryStore::default();
        store.add(make_memory(0, Decimal::new(9, 1), MemoryTier::LongTerm));
        store.add(make_memory(1, Decimal::new(8, 1), MemoryTier::LongTerm));

        let result = store.relevant_memories(10, Uuid::nil(), &[], &[]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn relevant_memories_includes_recent_immediate() {
        let mut store = MemoryStore::default();
        // Within the 5-tick window (tick 8, current_tick=10, cutoff=5)
        store.add(make_memory(8, Decimal::new(5, 1), MemoryTier::Immediate));
        // Outside the window
        store.add(make_memory(2, Decimal::new(5, 1), MemoryTier::Immediate));

        let result = store.relevant_memories(10, Uuid::nil(), &[], &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result.first().map(|e| e.tick), Some(8));
    }

    #[test]
    fn relevant_memories_filters_short_term_by_location() {
        let location = Uuid::new_v4();
        let other_location = Uuid::new_v4();

        let mut store = MemoryStore::default();
        // Relevant: references the current location
        store.add(make_memory_with_entities(
            5,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            vec![location],
        ));
        // Not relevant: references a different location
        store.add(make_memory_with_entities(
            6,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            vec![other_location],
        ));

        let result = store.relevant_memories(10, location, &[], &[]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn relevant_memories_filters_short_term_by_nearby_agents() {
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();

        let mut store = MemoryStore::default();
        // Relevant: references a nearby agent
        store.add(make_memory_with_entities(
            5,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            vec![agent_a],
        ));
        // Not relevant: references a different agent
        store.add(make_memory_with_entities(
            6,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            vec![agent_b],
        ));

        let result = store.relevant_memories(10, Uuid::nil(), &[agent_a], &[]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn relevant_memories_filters_short_term_by_goal_keywords() {
        let mut store = MemoryStore::default();
        // Relevant: summary matches "food" goal
        store.add(make_memory_with_summary(
            5,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            "Found a food source near the river",
        ));
        // Not relevant: summary does not match
        store.add(make_memory_with_summary(
            6,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            "Built a shelter from wood",
        ));

        let goals = vec![String::from("find food")];
        let result = store.relevant_memories(10, Uuid::nil(), &[], &goals);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn relevant_memories_respects_token_budget() {
        let config = MemoryConfig {
            max_memory_tokens: 10,
            ..MemoryConfig::default()
        };
        let mut store = MemoryStore::new(config);

        // Each memory: "Something happened nearby" = 25 chars -> 6 tokens
        store.add(make_memory(0, Decimal::new(9, 1), MemoryTier::LongTerm));
        store.add(make_memory(1, Decimal::new(9, 1), MemoryTier::LongTerm));
        store.add(make_memory(2, Decimal::new(9, 1), MemoryTier::LongTerm));

        let result = store.relevant_memories(10, Uuid::nil(), &[], &[]);
        // Budget = 10 tokens, each entry = 6 tokens.
        // First: 6 <= 10 -> include, remaining = 4
        // Second: 6 > 4 -> stop
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn relevant_memories_priority_long_term_first() {
        let config = MemoryConfig {
            max_memory_tokens: 20,
            ..MemoryConfig::default()
        };
        let mut store = MemoryStore::new(config);

        // Long-term memory (priority 1)
        store.add(make_memory_with_summary(
            0,
            Decimal::new(9, 1),
            MemoryTier::LongTerm,
            "Discovered fire for the first time",
        ));
        // Immediate memory (priority 2)
        store.add(make_memory_with_summary(
            9,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Gathered berries this morning",
        ));

        let result = store.relevant_memories(10, Uuid::nil(), &[], &[]);
        // Long-term should come first
        assert!(result.len() >= 1);
        let first = result.first();
        assert!(first.is_some());
        if let Some(entry) = first {
            assert_eq!(entry.tier, MemoryTier::LongTerm);
        }
    }

    #[test]
    fn relevant_memories_empty_store_returns_empty() {
        let store = MemoryStore::default();
        let result = store.relevant_memories(10, Uuid::nil(), &[], &[]);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // Memory persistence across ticks
    // -----------------------------------------------------------------------

    #[test]
    fn memories_persist_across_ticks() {
        let mut store = MemoryStore::default();

        // Tick 1: add a memory
        store.add(MemoryEntry::action(
            1,
            String::from("Gathered wood"),
            Vec::new(),
            Decimal::new(5, 1),
        ));

        // Tick 2: add another memory
        store.add(MemoryEntry::observation(
            2,
            String::from("Saw another agent"),
            Vec::new(),
            Decimal::new(4, 1),
        ));

        // Both still present at tick 3 (within immediate window)
        store.compress(3);
        assert_eq!(store.len(), 2);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 2);

        // At tick 10, they age out of immediate
        store.compress(10);
        // 0.5 -> short-term, 0.4 -> short-term (both >= 0.3)
        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 2);
    }

    // -----------------------------------------------------------------------
    // Configurable retention periods
    // -----------------------------------------------------------------------

    #[test]
    fn custom_retention_periods() {
        let config = MemoryConfig {
            immediate_retention_ticks: 3,
            short_term_retention_ticks: 10,
            max_memory_tokens: 2000,
        };
        let mut store = MemoryStore::new(config);

        // Tick 0: add memory
        store.add(make_memory(0, Decimal::new(5, 1), MemoryTier::Immediate));

        // At tick 2, still within 3-tick window (cutoff = 2 - 3 = 0 saturated, tick 0 not < 0)
        store.compress(2);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 1);

        // At tick 4, outside 3-tick window (cutoff = 4 - 3 = 1, tick 0 < 1)
        store.compress(4);
        assert_eq!(store.count_tier(MemoryTier::ShortTerm), 1);
        assert_eq!(store.count_tier(MemoryTier::Immediate), 0);

        // At tick 15, outside 10-tick window (cutoff = 15 - 10 = 5, tick 0 < 5)
        // Weight 0.5 not > 0.7, so discarded
        store.compress(15);
        assert!(store.is_empty());
    }

    // -----------------------------------------------------------------------
    // Importance scoring (Phase 8.4.2)
    // -----------------------------------------------------------------------

    #[test]
    fn importance_score_social_keywords() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Formed an alliance with Kora against the raiders",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_SOCIAL).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_combat_keywords() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Was attacked by a hostile agent near the river",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_COMBAT).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_discovery_keywords() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Discovered how to smelt iron ore",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_DISCOVERY).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_routine_keywords() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Gathered berries from the bush",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_ROUTINE).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_default_for_unrecognized() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Stood silently contemplating the horizon",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_DEFAULT).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_case_insensitive() {
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "BETRAYED by former ally during the night",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_SOCIAL).abs() < f64::EPSILON);
    }

    #[test]
    fn importance_score_highest_category_wins() {
        // "trade" is social (3.0), "gather" is routine (1.0) -- social should win
        let entry = make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::Immediate,
            "Attempted to trade while gathering resources",
        );
        let score = importance_score(&entry);
        assert!((score - IMPORTANCE_SOCIAL).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Reflection triggers (Phase 8.4.1)
    // -----------------------------------------------------------------------

    #[test]
    fn reflection_triggers_location_match() {
        let memories = vec![
            make_memory_with_summary(
                1,
                Decimal::new(9, 1),
                MemoryTier::LongTerm,
                "Was betrayed at the River Crossing by an ally",
            ),
            make_memory_with_summary(
                2,
                Decimal::new(5, 1),
                MemoryTier::Immediate,
                "Gathered berries near the forest",
            ),
        ];

        let triggers = find_reflection_triggers(
            &memories,
            "River Crossing",
            &[],
            5,
        );
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers.first().map(|e| e.tick), Some(1));
    }

    #[test]
    fn reflection_triggers_agent_name_match() {
        let memories = vec![
            make_memory_with_summary(
                5,
                Decimal::new(8, 1),
                MemoryTier::LongTerm,
                "Kora attacked me and stole my food",
            ),
            make_memory_with_summary(
                6,
                Decimal::new(5, 1),
                MemoryTier::ShortTerm,
                "Talked to Mira about the weather",
            ),
        ];

        let triggers = find_reflection_triggers(
            &memories,
            "Plains",
            &[String::from("Kora"), String::from("Zev")],
            5,
        );
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers.first().map(|e| e.tick), Some(5));
    }

    #[test]
    fn reflection_triggers_excludes_low_importance() {
        // "Gathered" is routine (1.0) -- below 2.5 threshold
        let memories = vec![make_memory_with_summary(
            1,
            Decimal::new(5, 1),
            MemoryTier::ShortTerm,
            "Gathered wood at the River Crossing",
        )];

        let triggers = find_reflection_triggers(
            &memories,
            "River Crossing",
            &[],
            5,
        );
        assert!(triggers.is_empty());
    }

    #[test]
    fn reflection_triggers_respects_max_results() {
        let memories = vec![
            make_memory_with_summary(
                1,
                Decimal::new(9, 1),
                MemoryTier::LongTerm,
                "Alliance formed at Village Square",
            ),
            make_memory_with_summary(
                2,
                Decimal::new(9, 1),
                MemoryTier::LongTerm,
                "War declared at Village Square",
            ),
            make_memory_with_summary(
                3,
                Decimal::new(9, 1),
                MemoryTier::LongTerm,
                "Trade dispute at Village Square",
            ),
        ];

        let triggers = find_reflection_triggers(
            &memories,
            "Village Square",
            &[],
            2,
        );
        assert_eq!(triggers.len(), 2);
    }

    #[test]
    fn reflection_triggers_empty_context_returns_empty() {
        let memories = vec![make_memory_with_summary(
            1,
            Decimal::new(9, 1),
            MemoryTier::LongTerm,
            "Betrayed at the River Crossing",
        )];

        let triggers = find_reflection_triggers(&memories, "", &[], 5);
        assert!(triggers.is_empty());
    }

    #[test]
    fn reflection_triggers_empty_memories_returns_empty() {
        let triggers = find_reflection_triggers(
            &[],
            "River Crossing",
            &[String::from("Kora")],
            5,
        );
        assert!(triggers.is_empty());
    }

    // -----------------------------------------------------------------------
    // Compression record (Phase 8.4.3)
    // -----------------------------------------------------------------------

    #[test]
    fn compress_returns_record_with_counts() {
        let mut store = MemoryStore::default();
        // High weight -> promoted to long-term
        store.add(make_memory(0, Decimal::new(8, 1), MemoryTier::Immediate));
        // Low weight -> discarded
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::Immediate));

        let record = store.compress(10);

        assert_eq!(record.tick, 10);
        assert_eq!(record.original_count, 2);
        assert_eq!(record.compressed_count, 1);
        assert_eq!(record.importance_scores_dropped.len(), 1);
    }

    #[test]
    fn compress_record_no_changes() {
        let mut store = MemoryStore::default();
        // Recent memory -- not yet eligible for compression
        store.add(make_memory(9, Decimal::new(5, 1), MemoryTier::Immediate));

        let record = store.compress(10);

        assert_eq!(record.original_count, 1);
        assert_eq!(record.compressed_count, 1);
        assert!(record.importance_scores_dropped.is_empty());
    }

    #[test]
    fn compress_record_summary_is_descriptive() {
        let mut store = MemoryStore::default();
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::Immediate));

        let record = store.compress(10);

        assert!(record.summary.contains("tick 10"));
        assert!(record.summary.contains("1 dropped"));
    }

    #[test]
    fn compress_record_importance_scores_of_dropped() {
        let mut store = MemoryStore::default();
        // "Something happened nearby" has no special keywords -> default 1.5
        store.add(make_memory(0, Decimal::new(1, 1), MemoryTier::Immediate));
        store.add(make_memory(0, Decimal::new(2, 1), MemoryTier::Immediate));

        let record = store.compress(10);

        assert_eq!(record.importance_scores_dropped.len(), 2);
        // Both have the default "Something happened nearby" summary -> 1.5
        for &score in &record.importance_scores_dropped {
            assert!((score - IMPORTANCE_DEFAULT).abs() < f64::EPSILON);
        }
    }
}
