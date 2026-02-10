//! Technology and cultural knowledge diffusion tracking.
//!
//! Tracks how knowledge (both technical and cultural) spreads through the
//! population over time. Provides adoption curves, penetration metrics,
//! resistance tracking, diffusion speed measurement, and identification of
//! knowledge hoarders and innovation leaders.
//!
//! # Core Concepts
//!
//! - **Adoption**: when an agent acquires a piece of knowledge.
//! - **Resistance**: when an agent is exposed to knowledge but does not adopt it.
//! - **Diffusion speed**: ticks from first adoption to 50% population penetration.
//! - **Knowledge hoarders**: agents who hold knowledge that very few others possess.
//! - **Innovation leaders**: agents with the most independent discoveries.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use emergence_types::AgentId;

// ---------------------------------------------------------------------------
// Diffusion Source
// ---------------------------------------------------------------------------

/// How an agent acquired a piece of knowledge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiffusionSource {
    /// The agent discovered or created the knowledge independently.
    Independent,
    /// The agent was taught by another agent.
    Taught {
        /// The agent who taught this knowledge.
        teacher: AgentId,
    },
    /// The agent learned by observing nearby agents.
    Observed,
    /// The knowledge was passed from a parent at birth.
    Inherited,
    /// The knowledge was acquired through a trade interaction.
    Trade,
}

// ---------------------------------------------------------------------------
// Diffusion Event
// ---------------------------------------------------------------------------

/// A single adoption event: one agent learned one piece of knowledge at a tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffusionEvent {
    /// The knowledge item that was adopted.
    pub knowledge_id: String,
    /// The agent who adopted it.
    pub agent_id: AgentId,
    /// The tick when adoption occurred.
    pub tick: u64,
    /// How the agent acquired this knowledge.
    pub source: DiffusionSource,
}

// ---------------------------------------------------------------------------
// Resistance Record
// ---------------------------------------------------------------------------

/// A record of an agent being exposed to knowledge but rejecting it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResistanceRecord {
    /// The knowledge item that was rejected.
    pub knowledge_id: String,
    /// The agent who rejected it.
    pub agent_id: AgentId,
    /// The tick when rejection occurred.
    pub tick: u64,
    /// Optional reason for rejection (personality mismatch, conflicting knowledge, etc.).
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Adoption Curve
// ---------------------------------------------------------------------------

/// The adoption curve for a single knowledge item over time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdoptionCurve {
    /// The knowledge item this curve describes.
    pub knowledge_id: String,
    /// The tick when the first agent adopted this knowledge.
    pub first_adoption_tick: u64,
    /// Cumulative adoption over time: `(tick, cumulative_adopters)` pairs.
    pub adoption_by_tick: Vec<(u64, u32)>,
    /// Total number of agents who have adopted this knowledge (ever).
    pub total_adopters: u32,
    /// Maximum new adopters recorded in a single tick.
    pub peak_adoption_rate: f64,
}

// ---------------------------------------------------------------------------
// Source Breakdown
// ---------------------------------------------------------------------------

/// Breakdown of adoption sources for a knowledge item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceBreakdown {
    /// Number of independent discoveries.
    pub independent: u32,
    /// Number of teaching-based adoptions.
    pub taught: u32,
    /// Number of observation-based adoptions.
    pub observed: u32,
    /// Number of inherited adoptions (from parent).
    pub inherited: u32,
    /// Number of trade-based adoptions.
    pub trade: u32,
}

// ---------------------------------------------------------------------------
// Diffusion Tracker
// ---------------------------------------------------------------------------

/// Central tracker for knowledge diffusion across the simulation.
///
/// Records every adoption and rejection event, and provides analytics
/// methods for computing adoption curves, penetration rates, diffusion
/// speed, and identifying knowledge hoarders and innovation leaders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffusionTracker {
    /// All adoption events, keyed by knowledge ID.
    adoptions: HashMap<String, Vec<DiffusionEvent>>,
    /// All resistance records, keyed by knowledge ID.
    rejections: HashMap<String, Vec<ResistanceRecord>>,
    /// Set of agents who currently hold each knowledge item.
    current_holders: HashMap<String, HashSet<AgentId>>,
    /// Per-knowledge-item, per-tick count of new adoptions.
    adoptions_per_tick: HashMap<String, BTreeMap<u64, u32>>,
}

impl DiffusionTracker {
    /// Create an empty diffusion tracker.
    pub fn new() -> Self {
        Self {
            adoptions: HashMap::new(),
            rejections: HashMap::new(),
            current_holders: HashMap::new(),
            adoptions_per_tick: HashMap::new(),
        }
    }

    /// Record that an agent adopted a piece of knowledge.
    pub fn record_adoption(&mut self, event: DiffusionEvent) {
        let kid = event.knowledge_id.clone();
        let agent = event.agent_id;
        let tick = event.tick;

        // Update current holders.
        self.current_holders
            .entry(kid.clone())
            .or_default()
            .insert(agent);

        // Update per-tick adoption count.
        let tick_map = self.adoptions_per_tick.entry(kid.clone()).or_default();
        let count = tick_map.entry(tick).or_insert(0);
        *count = count.saturating_add(1);

        // Store the event.
        self.adoptions.entry(kid).or_default().push(event);
    }

    /// Record that an agent was exposed to knowledge but did not adopt it.
    pub fn record_rejection(&mut self, record: ResistanceRecord) {
        let kid = record.knowledge_id.clone();
        self.rejections.entry(kid).or_default().push(record);
    }

    /// Build the [`AdoptionCurve`] for a knowledge item.
    ///
    /// Returns `None` if no adoption events exist for the given knowledge ID.
    pub fn get_adoption_curve(&self, knowledge_id: &str) -> Option<AdoptionCurve> {
        let tick_map = self.adoptions_per_tick.get(knowledge_id)?;
        if tick_map.is_empty() {
            return None;
        }

        let mut cumulative: u32 = 0;
        let mut peak_rate: f64 = 0.0;
        let mut adoption_by_tick: Vec<(u64, u32)> = Vec::with_capacity(tick_map.len());
        let mut first_tick: Option<u64> = None;

        for (&tick, &count) in tick_map {
            if first_tick.is_none() {
                first_tick = Some(tick);
            }
            cumulative = cumulative.saturating_add(count);
            adoption_by_tick.push((tick, cumulative));

            let count_f64 = f64::from(count);
            if count_f64 > peak_rate {
                peak_rate = count_f64;
            }
        }

        Some(AdoptionCurve {
            knowledge_id: String::from(knowledge_id),
            first_adoption_tick: first_tick.unwrap_or(0),
            adoption_by_tick,
            total_adopters: cumulative,
            peak_adoption_rate: peak_rate,
        })
    }

    /// Compute the current adoption rate (new adopters per tick) over a
    /// recent window of `window_size` ticks ending at `current_tick`.
    ///
    /// Returns 0.0 if no data exists or window size is zero.
    pub fn adoption_rate(&self, knowledge_id: &str, current_tick: u64, window_size: u64) -> f64 {
        if window_size == 0 {
            return 0.0;
        }

        let Some(tick_map) = self.adoptions_per_tick.get(knowledge_id) else {
            return 0.0;
        };

        let window_start = current_tick.saturating_sub(window_size);
        let mut total_in_window: u64 = 0;

        for (&tick, &count) in tick_map {
            if tick > window_start && tick <= current_tick {
                total_in_window = total_in_window.saturating_add(u64::from(count));
            }
        }

        // Both values are bounded; safe to represent as f64.
        #[allow(clippy::cast_precision_loss)]
        let rate = total_in_window as f64 / window_size as f64;
        rate
    }

    /// Compute the percentage of living agents who currently hold this knowledge.
    ///
    /// `total_living` is the current number of living agents in the simulation.
    /// Returns 0.0 if `total_living` is zero.
    pub fn population_penetration(&self, knowledge_id: &str, total_living: u32) -> f64 {
        if total_living == 0 {
            return 0.0;
        }

        let holder_count = self
            .current_holders
            .get(knowledge_id)
            .map_or(0, HashSet::len);

        // Both values are bounded; safe to represent as f64.
        #[allow(clippy::cast_precision_loss)]
        let penetration = holder_count as f64 / f64::from(total_living);
        penetration
    }

    /// Compute diffusion speed: ticks from first adoption to 50% population penetration.
    ///
    /// Returns `None` if the knowledge hasn't reached 50% penetration.
    /// `total_living` is the current population count.
    pub fn diffusion_speed(&self, knowledge_id: &str, total_living: u32) -> Option<u64> {
        if total_living == 0 {
            return None;
        }

        let tick_map = self.adoptions_per_tick.get(knowledge_id)?;
        if tick_map.is_empty() {
            return None;
        }

        // half_pop uses integer division, rounding down.
        let half_pop = total_living.checked_div(2).unwrap_or(0);
        if half_pop == 0 {
            // Population of 1 -- any adoption counts as 100%.
            let first_tick = tick_map.keys().copied().next()?;
            return Some(0_u64.saturating_sub(first_tick).saturating_add(first_tick).saturating_sub(first_tick));
        }

        let mut cumulative: u32 = 0;
        let mut first_tick: Option<u64> = None;

        for (&tick, &count) in tick_map {
            if first_tick.is_none() {
                first_tick = Some(tick);
            }
            cumulative = cumulative.saturating_add(count);
            if cumulative >= half_pop {
                let start = first_tick.unwrap_or(tick);
                return Some(tick.saturating_sub(start));
            }
        }

        None
    }

    /// Identify knowledge hoarders: agents who hold items known by fewer than
    /// `threshold_pct` percent of the population.
    ///
    /// Returns a map of agent ID to the list of rare knowledge items they hold.
    /// `total_living` is the current population count.
    pub fn get_knowledge_hoarders(
        &self,
        threshold_pct: f64,
        total_living: u32,
    ) -> BTreeMap<AgentId, Vec<String>> {
        if total_living == 0 {
            return BTreeMap::new();
        }

        let mut result: BTreeMap<AgentId, Vec<String>> = BTreeMap::new();

        for (kid, holders) in &self.current_holders {
            let holder_count = holders.len();

            #[allow(clippy::cast_precision_loss)]
            let pct = holder_count as f64 / f64::from(total_living);

            if pct < threshold_pct {
                for &agent_id in holders {
                    result.entry(agent_id).or_default().push(kid.clone());
                }
            }
        }

        result
    }

    /// Compute the resistance rate for a knowledge item: the fraction of
    /// exposures (adoptions + rejections) that resulted in rejection.
    ///
    /// Returns 0.0 if there have been no exposures.
    pub fn resistance_rate(&self, knowledge_id: &str) -> f64 {
        let adoption_count = self
            .adoptions
            .get(knowledge_id)
            .map_or(0_u64, |v| {
                let len = v.len();
                // Vec::len() returns usize; on all supported platforms this fits in u64.
                #[allow(clippy::cast_possible_truncation)]
                let len_u64 = len as u64;
                len_u64
            });
        let rejection_count = self
            .rejections
            .get(knowledge_id)
            .map_or(0_u64, |v| {
                let len = v.len();
                #[allow(clippy::cast_possible_truncation)]
                let len_u64 = len as u64;
                len_u64
            });

        let total = adoption_count.saturating_add(rejection_count);
        if total == 0 {
            return 0.0;
        }

        #[allow(clippy::cast_precision_loss)]
        let rate = rejection_count as f64 / total as f64;
        rate
    }

    /// Return knowledge items ranked by diffusion speed (fastest first).
    ///
    /// Only includes items that have reached 50% penetration.
    /// `total_living` is the current population count.
    pub fn get_fastest_spreading(&self, total_living: u32) -> Vec<(String, u64)> {
        let mut items: Vec<(String, u64)> = self
            .adoptions_per_tick
            .keys()
            .filter_map(|kid| {
                let speed = self.diffusion_speed(kid, total_living)?;
                Some((kid.clone(), speed))
            })
            .collect();
        items.sort_by_key(|&(_, speed)| speed);
        items
    }

    /// Return knowledge items ranked by how slowly they spread.
    ///
    /// Items are ranked by resistance rate (highest first). Includes items
    /// that may not have reached 50% penetration.
    pub fn get_slowest_spreading(&self) -> Vec<(String, f64)> {
        let mut all_kids: HashSet<&String> = HashSet::new();
        for kid in self.adoptions.keys() {
            all_kids.insert(kid);
        }
        for kid in self.rejections.keys() {
            all_kids.insert(kid);
        }

        let mut items: Vec<(String, f64)> = all_kids
            .into_iter()
            .map(|kid| {
                let rate = self.resistance_rate(kid);
                (kid.clone(), rate)
            })
            .collect();

        // Sort by resistance rate descending. Using total_cmp for deterministic ordering.
        items.sort_by(|a, b| b.1.total_cmp(&a.1));
        items
    }

    /// Return the source breakdown for a knowledge item: how many adoptions
    /// came from each source type.
    pub fn adoption_by_source(&self, knowledge_id: &str) -> SourceBreakdown {
        let Some(events) = self.adoptions.get(knowledge_id) else {
            return SourceBreakdown::default();
        };

        let mut breakdown = SourceBreakdown::default();
        for event in events {
            match &event.source {
                DiffusionSource::Independent => {
                    breakdown.independent = breakdown.independent.saturating_add(1);
                }
                DiffusionSource::Taught { .. } => {
                    breakdown.taught = breakdown.taught.saturating_add(1);
                }
                DiffusionSource::Observed => {
                    breakdown.observed = breakdown.observed.saturating_add(1);
                }
                DiffusionSource::Inherited => {
                    breakdown.inherited = breakdown.inherited.saturating_add(1);
                }
                DiffusionSource::Trade => {
                    breakdown.trade = breakdown.trade.saturating_add(1);
                }
            }
        }
        breakdown
    }

    /// Identify innovation leaders: agents ranked by number of independent
    /// discoveries (most first).
    pub fn get_innovation_leaders(&self) -> Vec<(AgentId, u32)> {
        let mut counts: HashMap<AgentId, u32> = HashMap::new();
        for events in self.adoptions.values() {
            for event in events {
                if event.source == DiffusionSource::Independent {
                    let count = counts.entry(event.agent_id).or_insert(0);
                    *count = count.saturating_add(1);
                }
            }
        }

        let mut leaders: Vec<(AgentId, u32)> = counts.into_iter().collect();
        leaders.sort_by(|a, b| b.1.cmp(&a.1));
        leaders
    }

    /// Return the number of adoption events recorded for a knowledge item.
    pub fn adoption_count(&self, knowledge_id: &str) -> usize {
        self.adoptions.get(knowledge_id).map_or(0, Vec::len)
    }

    /// Return the number of rejection events recorded for a knowledge item.
    pub fn rejection_count(&self, knowledge_id: &str) -> usize {
        self.rejections.get(knowledge_id).map_or(0, Vec::len)
    }

    /// Return the current number of agents who hold a knowledge item.
    pub fn current_holder_count(&self, knowledge_id: &str) -> usize {
        self.current_holders
            .get(knowledge_id)
            .map_or(0, HashSet::len)
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

    fn make_adoption(kid: &str, agent: AgentId, tick: u64, source: DiffusionSource) -> DiffusionEvent {
        DiffusionEvent {
            knowledge_id: String::from(kid),
            agent_id: agent,
            tick,
            source,
        }
    }

    fn make_rejection(kid: &str, agent: AgentId, tick: u64) -> ResistanceRecord {
        ResistanceRecord {
            knowledge_id: String::from(kid),
            agent_id: agent,
            tick,
            reason: None,
        }
    }

    // ------------------------------------------------------------------
    // Adoption recording
    // ------------------------------------------------------------------

    #[test]
    fn record_adoption_basic() {
        let mut tracker = DiffusionTracker::new();
        let agent = make_agent();
        let event = make_adoption("fire", agent, 1, DiffusionSource::Independent);
        tracker.record_adoption(event);

        assert_eq!(tracker.adoption_count("fire"), 1);
        assert_eq!(tracker.current_holder_count("fire"), 1);
    }

    #[test]
    fn record_multiple_adoptions_same_tick() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        let b = make_agent();
        let c = make_agent();

        tracker.record_adoption(make_adoption("fire", a, 5, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("fire", b, 5, DiffusionSource::Taught { teacher: a }));
        tracker.record_adoption(make_adoption("fire", c, 5, DiffusionSource::Observed));

        assert_eq!(tracker.adoption_count("fire"), 3);
        assert_eq!(tracker.current_holder_count("fire"), 3);
    }

    // ------------------------------------------------------------------
    // Adoption curve
    // ------------------------------------------------------------------

    #[test]
    fn adoption_curve_generation() {
        let mut tracker = DiffusionTracker::new();
        let agents: Vec<AgentId> = (0..5).map(|_| make_agent()).collect();

        // Tick 1: 1 adopter, Tick 3: 2 adopters, Tick 5: 2 adopters.
        tracker.record_adoption(make_adoption("wheel", agents[0], 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("wheel", agents[1], 3, DiffusionSource::Taught { teacher: agents[0] }));
        tracker.record_adoption(make_adoption("wheel", agents[2], 3, DiffusionSource::Observed));
        tracker.record_adoption(make_adoption("wheel", agents[3], 5, DiffusionSource::Taught { teacher: agents[1] }));
        tracker.record_adoption(make_adoption("wheel", agents[4], 5, DiffusionSource::Trade));

        let curve = tracker.get_adoption_curve("wheel");
        assert!(curve.is_some());
        let curve = curve.unwrap_or_else(|| AdoptionCurve {
            knowledge_id: String::new(),
            first_adoption_tick: 0,
            adoption_by_tick: vec![],
            total_adopters: 0,
            peak_adoption_rate: 0.0,
        });

        assert_eq!(curve.knowledge_id, "wheel");
        assert_eq!(curve.first_adoption_tick, 1);
        assert_eq!(curve.total_adopters, 5);
        // Peak adoption rate: 2 adopters in tick 3 and tick 5.
        assert!((curve.peak_adoption_rate - 2.0).abs() < 0.01);

        // Check cumulative values: tick 1 -> 1, tick 3 -> 3, tick 5 -> 5.
        assert_eq!(curve.adoption_by_tick.len(), 3);
    }

    #[test]
    fn adoption_curve_nonexistent_returns_none() {
        let tracker = DiffusionTracker::new();
        assert!(tracker.get_adoption_curve("nonexistent").is_none());
    }

    // ------------------------------------------------------------------
    // Population penetration
    // ------------------------------------------------------------------

    #[test]
    fn population_penetration_calculation() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        let b = make_agent();

        tracker.record_adoption(make_adoption("fire", a, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("fire", b, 2, DiffusionSource::Taught { teacher: a }));

        // 2 holders out of 10 living agents = 20%.
        let pen = tracker.population_penetration("fire", 10);
        assert!((pen - 0.2).abs() < 0.01);
    }

    #[test]
    fn population_penetration_zero_population() {
        let tracker = DiffusionTracker::new();
        let pen = tracker.population_penetration("fire", 0);
        assert!(pen.abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Diffusion speed
    // ------------------------------------------------------------------

    #[test]
    fn diffusion_speed_basic() {
        let mut tracker = DiffusionTracker::new();
        let agents: Vec<AgentId> = (0..6).map(|_| make_agent()).collect();

        // First adoption at tick 10.
        tracker.record_adoption(make_adoption("pottery", agents[0], 10, DiffusionSource::Independent));
        // More adoptions spread over time.
        tracker.record_adoption(make_adoption("pottery", agents[1], 12, DiffusionSource::Taught { teacher: agents[0] }));
        tracker.record_adoption(make_adoption("pottery", agents[2], 14, DiffusionSource::Observed));
        tracker.record_adoption(make_adoption("pottery", agents[3], 16, DiffusionSource::Taught { teacher: agents[1] }));
        tracker.record_adoption(make_adoption("pottery", agents[4], 18, DiffusionSource::Trade));
        tracker.record_adoption(make_adoption("pottery", agents[5], 20, DiffusionSource::Observed));

        // Total living = 10. 50% = 5. Cumulative reaches 5 at tick 18.
        // Speed = 18 - 10 = 8 ticks.
        let speed = tracker.diffusion_speed("pottery", 10);
        assert_eq!(speed, Some(8));
    }

    #[test]
    fn diffusion_speed_not_reached() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        tracker.record_adoption(make_adoption("rare_item", a, 1, DiffusionSource::Independent));

        // 1 holder, 100 living. 50% = 50. Not reached.
        assert!(tracker.diffusion_speed("rare_item", 100).is_none());
    }

    // ------------------------------------------------------------------
    // Knowledge hoarders
    // ------------------------------------------------------------------

    #[test]
    fn knowledge_hoarders_detection() {
        let mut tracker = DiffusionTracker::new();
        let hoarder = make_agent();
        let common_agent = make_agent();

        // "rare_tech" known only by hoarder.
        tracker.record_adoption(make_adoption("rare_tech", hoarder, 1, DiffusionSource::Independent));
        // "common_tech" known by many.
        for i in 0..8 {
            let a = if i == 0 { common_agent } else { make_agent() };
            tracker.record_adoption(make_adoption("common_tech", a, 1, DiffusionSource::Independent));
        }

        // Total living = 10. Threshold = 20% = 0.2.
        // rare_tech: 1/10 = 10% < 20% => hoarder is a knowledge hoarder.
        let hoarders = tracker.get_knowledge_hoarders(0.2, 10);
        assert!(hoarders.contains_key(&hoarder));
        let hoarder_items = hoarders.get(&hoarder);
        assert!(hoarder_items.is_some());
        if let Some(items) = hoarder_items {
            assert!(items.contains(&String::from("rare_tech")));
        }
    }

    // ------------------------------------------------------------------
    // Resistance tracking
    // ------------------------------------------------------------------

    #[test]
    fn resistance_rate_calculation() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        let b = make_agent();
        let c = make_agent();
        let d = make_agent();

        // 2 adoptions, 2 rejections => resistance rate = 50%.
        tracker.record_adoption(make_adoption("controversial", a, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("controversial", b, 2, DiffusionSource::Taught { teacher: a }));
        tracker.record_rejection(make_rejection("controversial", c, 2));
        tracker.record_rejection(make_rejection("controversial", d, 3));

        let rate = tracker.resistance_rate("controversial");
        assert!((rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn resistance_rate_no_exposures() {
        let tracker = DiffusionTracker::new();
        let rate = tracker.resistance_rate("nothing");
        assert!(rate.abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Source breakdown
    // ------------------------------------------------------------------

    #[test]
    fn adoption_by_source_breakdown() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        let b = make_agent();
        let c = make_agent();
        let d = make_agent();
        let e = make_agent();

        tracker.record_adoption(make_adoption("tool", a, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("tool", b, 2, DiffusionSource::Taught { teacher: a }));
        tracker.record_adoption(make_adoption("tool", c, 3, DiffusionSource::Observed));
        tracker.record_adoption(make_adoption("tool", d, 4, DiffusionSource::Inherited));
        tracker.record_adoption(make_adoption("tool", e, 5, DiffusionSource::Trade));

        let breakdown = tracker.adoption_by_source("tool");
        assert_eq!(breakdown.independent, 1);
        assert_eq!(breakdown.taught, 1);
        assert_eq!(breakdown.observed, 1);
        assert_eq!(breakdown.inherited, 1);
        assert_eq!(breakdown.trade, 1);
    }

    #[test]
    fn adoption_by_source_nonexistent() {
        let tracker = DiffusionTracker::new();
        let breakdown = tracker.adoption_by_source("nothing");
        assert_eq!(breakdown.independent, 0);
        assert_eq!(breakdown.taught, 0);
    }

    // ------------------------------------------------------------------
    // Innovation leaders
    // ------------------------------------------------------------------

    #[test]
    fn innovation_leaders_ranking() {
        let mut tracker = DiffusionTracker::new();
        let genius = make_agent();
        let average = make_agent();
        let learner = make_agent();

        // Genius has 3 independent discoveries.
        tracker.record_adoption(make_adoption("fire", genius, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("wheel", genius, 5, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("pottery", genius, 10, DiffusionSource::Independent));

        // Average has 1 independent discovery.
        tracker.record_adoption(make_adoption("rope", average, 3, DiffusionSource::Independent));

        // Learner has 0 independent discoveries (all taught).
        tracker.record_adoption(make_adoption("fire", learner, 2, DiffusionSource::Taught { teacher: genius }));

        let leaders = tracker.get_innovation_leaders();
        assert_eq!(leaders.len(), 2);

        // First should be genius with 3.
        let first = leaders.first();
        assert!(first.is_some());
        if let Some((agent, count)) = first {
            assert_eq!(*agent, genius);
            assert_eq!(*count, 3);
        }
    }

    // ------------------------------------------------------------------
    // Adoption rate over window
    // ------------------------------------------------------------------

    #[test]
    fn adoption_rate_over_window() {
        let mut tracker = DiffusionTracker::new();
        let agents: Vec<AgentId> = (0..4).map(|_| make_agent()).collect();

        // Tick 8: 1, Tick 9: 1, Tick 10: 2
        tracker.record_adoption(make_adoption("bronze", agents[0], 8, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("bronze", agents[1], 9, DiffusionSource::Taught { teacher: agents[0] }));
        tracker.record_adoption(make_adoption("bronze", agents[2], 10, DiffusionSource::Observed));
        tracker.record_adoption(make_adoption("bronze", agents[3], 10, DiffusionSource::Trade));

        // Window of 5 ticks ending at tick 10 (ticks 6-10).
        // Adoptions in window: tick 8 (1) + tick 9 (1) + tick 10 (2) = 4.
        // Rate = 4 / 5 = 0.8 per tick.
        let rate = tracker.adoption_rate("bronze", 10, 5);
        assert!((rate - 0.8).abs() < 0.01);
    }

    #[test]
    fn adoption_rate_zero_window() {
        let tracker = DiffusionTracker::new();
        let rate = tracker.adoption_rate("anything", 10, 0);
        assert!(rate.abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Fastest / Slowest spreading
    // ------------------------------------------------------------------

    #[test]
    fn fastest_spreading_ordering() {
        let mut tracker = DiffusionTracker::new();

        // "fast_item": reaches 50% of 4 (i.e., 2 adopters) in 2 ticks.
        let a1 = make_agent();
        let a2 = make_agent();
        tracker.record_adoption(make_adoption("fast_item", a1, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("fast_item", a2, 3, DiffusionSource::Taught { teacher: a1 }));

        // "slow_item": reaches 50% of 4 in 8 ticks.
        let b1 = make_agent();
        let b2 = make_agent();
        tracker.record_adoption(make_adoption("slow_item", b1, 2, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("slow_item", b2, 10, DiffusionSource::Taught { teacher: b1 }));

        let fastest = tracker.get_fastest_spreading(4);
        // Both items should have reached 50% of 4 (2 adopters).
        assert_eq!(fastest.len(), 2);
        // First should be fast_item with speed = 3-1 = 2.
        let first = fastest.first();
        if let Some((kid, speed)) = first {
            assert_eq!(kid, "fast_item");
            assert_eq!(*speed, 2);
        }
    }

    #[test]
    fn slowest_spreading_by_resistance() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();
        let b = make_agent();
        let c = make_agent();

        // "hated_item": 1 adoption, 3 rejections => 75% resistance.
        tracker.record_adoption(make_adoption("hated_item", a, 1, DiffusionSource::Independent));
        tracker.record_rejection(make_rejection("hated_item", b, 2));
        tracker.record_rejection(make_rejection("hated_item", c, 3));
        tracker.record_rejection(make_rejection("hated_item", make_agent(), 4));

        // "liked_item": 2 adoptions, 0 rejections => 0% resistance.
        tracker.record_adoption(make_adoption("liked_item", b, 1, DiffusionSource::Independent));
        tracker.record_adoption(make_adoption("liked_item", c, 2, DiffusionSource::Taught { teacher: b }));

        let slowest = tracker.get_slowest_spreading();
        // First should be hated_item with 75% resistance.
        let first = slowest.first();
        if let Some((kid, rate)) = first {
            assert_eq!(kid, "hated_item");
            assert!((rate - 0.75).abs() < 0.01);
        }
    }

    // ------------------------------------------------------------------
    // Rejection recording
    // ------------------------------------------------------------------

    #[test]
    fn rejection_recording_and_count() {
        let mut tracker = DiffusionTracker::new();
        let a = make_agent();

        let record = ResistanceRecord {
            knowledge_id: String::from("strange_idea"),
            agent_id: a,
            tick: 5,
            reason: Some(String::from("personality mismatch")),
        };
        tracker.record_rejection(record);

        assert_eq!(tracker.rejection_count("strange_idea"), 1);
    }
}
