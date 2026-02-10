//! Propaganda system for the Emergence simulation.
//!
//! Agents can post persistent public declarations at locations that influence
//! newcomers' perception of local culture and norms. Propaganda is a form
//! of soft power -- it does not mechanically enforce rules but shapes
//! agent decision-making through information exposure.
//!
//! # Architecture
//!
//! Propaganda is location-bound. Each post is anchored to a specific location
//! and has a type (declaration, recruitment, warning, etc.), content string,
//! and influence strength derived from the author's reputation and charisma.
//!
//! Posts expire after a configurable number of ticks (default 500).
//! Counter-propaganda at the same location weakens existing posts of
//! opposing views.
//!
//! # Events
//!
//! - `PropagandaPosted` -- emitted when a new propaganda post is created.
//! - `PropagandaExpired` -- emitted when a post expires or is manually removed.
//!
//! # Invariants
//!
//! - Posts are append-only once created (content does not change).
//! - Expired posts are marked but not deleted (for historical analysis).
//! - Influence strength is clamped to [0.0, 1.0].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::{AgentId, LocationId};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default expiration age for propaganda posts in ticks.
const DEFAULT_EXPIRY_TICKS: u64 = 500;

/// Minimum influence strength (floor after counter-propaganda).
const MIN_INFLUENCE: f64 = 0.0;

/// Maximum influence strength.
const MAX_INFLUENCE: f64 = 1.0;

/// Amount by which counter-propaganda weakens existing posts.
const COUNTER_PROPAGANDA_PENALTY: f64 = 0.15;

// ---------------------------------------------------------------------------
// PropagandaType
// ---------------------------------------------------------------------------

/// The category of a propaganda post.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PropagandaType {
    /// Statement of rules or norms ("Stealing is punished here").
    Declaration,
    /// Call to join a group or belief ("Join the River Watchers").
    Recruitment,
    /// Threat or warning ("Outsiders beware").
    Warning,
    /// Praise for a leader or group ("Long live Chief Axar").
    Tribute,
    /// Narrative about past events ("We defeated the Stone Clan at tick 500").
    History,
    /// Religious or philosophical teaching ("The river provides all").
    Doctrine,
}

// ---------------------------------------------------------------------------
// PropagandaPost
// ---------------------------------------------------------------------------

/// A persistent propaganda post anchored to a location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropagandaPost {
    /// Unique post identifier.
    pub id: Uuid,
    /// The agent who created this post.
    pub author: AgentId,
    /// The location where this post is visible.
    pub location: LocationId,
    /// The tick when this post was created.
    pub tick_posted: u64,
    /// The category of propaganda.
    pub propaganda_type: PropagandaType,
    /// The content of the propaganda message.
    pub content: String,
    /// Optional linked social construct (religion, governance, etc.).
    pub associated_construct: Option<Uuid>,
    /// How persuasive this post is (0.0 to 1.0).
    pub influence_strength: f64,
    /// Whether this post has been expired (manually or by age).
    pub expired: bool,
}

// ---------------------------------------------------------------------------
// PropagandaInfluence
// ---------------------------------------------------------------------------

/// Aggregated influence of propaganda at a location, grouped by construct
/// or belief system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropagandaInfluence {
    /// The type of propaganda contributing to this influence.
    pub source_type: PropagandaType,
    /// A summary of the propaganda content.
    pub content_summary: String,
    /// Total aggregated influence from all matching posts.
    pub total_influence: f64,
    /// The associated social construct, if any.
    pub associated_construct: Option<Uuid>,
    /// Number of posts contributing to this influence.
    pub post_count: u32,
}

// ---------------------------------------------------------------------------
// PropagandaParams
// ---------------------------------------------------------------------------

/// Parameters for creating a propaganda post.
///
/// Bundles the arguments for [`PropagandaBoard::post_propaganda`] and
/// [`PropagandaBoard::counter_propaganda`] to satisfy the argument-count lint.
#[derive(Debug, Clone)]
pub struct PropagandaParams {
    /// The agent creating the post.
    pub author: AgentId,
    /// The location where the post will be visible.
    pub location: LocationId,
    /// The tick when the post is created.
    pub tick: u64,
    /// The category of propaganda.
    pub propaganda_type: PropagandaType,
    /// The content of the propaganda message.
    pub content: String,
    /// Optional linked social construct.
    pub associated_construct: Option<Uuid>,
    /// How persuasive this post is (0.0 to 1.0, clamped).
    pub influence_strength: f64,
}

// ---------------------------------------------------------------------------
// PropagandaBoard
// ---------------------------------------------------------------------------

/// Central registry for all propaganda posts in the simulation.
///
/// Posts are stored by their unique ID and indexed by location, author,
/// and type for efficient querying.
#[derive(Debug, Clone)]
pub struct PropagandaBoard {
    /// All posts, keyed by their unique ID.
    posts: BTreeMap<Uuid, PropagandaPost>,
    /// Index: location -> set of post IDs at that location.
    location_index: BTreeMap<LocationId, BTreeSet<Uuid>>,
    /// Index: author -> set of post IDs by that author.
    author_index: BTreeMap<AgentId, BTreeSet<Uuid>>,
    /// Tracks which agents have visited which locations since a given tick.
    /// Used for reach estimation: location -> (`agent_id`, `first_visit_tick`).
    visit_log: BTreeMap<LocationId, BTreeMap<AgentId, u64>>,
}

impl PropagandaBoard {
    /// Create a new empty propaganda board.
    pub const fn new() -> Self {
        Self {
            posts: BTreeMap::new(),
            location_index: BTreeMap::new(),
            author_index: BTreeMap::new(),
            visit_log: BTreeMap::new(),
        }
    }

    /// Create a new propaganda post at a location.
    ///
    /// The `influence_strength` is clamped to [0.0, 1.0].
    /// Returns the ID of the newly created post.
    pub fn post_propaganda(&mut self, params: &PropagandaParams) -> Uuid {
        let id = Uuid::now_v7();
        let clamped_influence = clamp_influence(params.influence_strength);

        let post = PropagandaPost {
            id,
            author: params.author,
            location: params.location,
            tick_posted: params.tick,
            propaganda_type: params.propaganda_type,
            content: params.content.clone(),
            associated_construct: params.associated_construct,
            influence_strength: clamped_influence,
            expired: false,
        };

        self.posts.insert(id, post);
        self.location_index
            .entry(params.location)
            .or_default()
            .insert(id);
        self.author_index
            .entry(params.author)
            .or_default()
            .insert(id);

        id
    }

    /// Get all active (non-expired) posts at a location.
    pub fn get_posts_at_location(&self, location: LocationId) -> Vec<&PropagandaPost> {
        self.location_index
            .get(&location)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.posts.get(id))
                    .filter(|p| !p.expired)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all posts by a specific author (including expired).
    pub fn get_posts_by_author(&self, author: AgentId) -> Vec<&PropagandaPost> {
        self.author_index
            .get(&author)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.posts.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all active posts of a specific type across all locations.
    pub fn get_posts_by_type(&self, propaganda_type: PropagandaType) -> Vec<&PropagandaPost> {
        self.posts
            .values()
            .filter(|p| !p.expired && p.propaganda_type == propaganda_type)
            .collect()
    }

    /// Get all active posts associated with a specific social construct.
    pub fn get_posts_for_construct(&self, construct_id: Uuid) -> Vec<&PropagandaPost> {
        self.posts
            .values()
            .filter(|p| {
                !p.expired
                    && p.associated_construct
                        .is_some_and(|c| c == construct_id)
            })
            .collect()
    }

    /// Manually expire a specific post.
    ///
    /// Returns `Ok(())` if the post was found and expired, or an error
    /// if the post does not exist.
    pub fn expire_post(&mut self, post_id: Uuid) -> Result<(), AgentError> {
        let post = self.posts.get_mut(&post_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("propaganda post {post_id} not found"),
            }
        })?;
        post.expired = true;
        Ok(())
    }

    /// Automatically expire all posts older than `max_age_ticks`.
    ///
    /// Returns the number of posts that were expired.
    pub fn auto_expire(
        &mut self,
        current_tick: u64,
        max_age_ticks: Option<u64>,
    ) -> u32 {
        let max_age = max_age_ticks.unwrap_or(DEFAULT_EXPIRY_TICKS);
        let threshold_tick = current_tick.saturating_sub(max_age);
        let mut expired_count: u32 = 0;

        for post in self.posts.values_mut() {
            if !post.expired && post.tick_posted < threshold_tick {
                post.expired = true;
                expired_count = expired_count.saturating_add(1);
            }
        }

        expired_count
    }

    /// Compute the aggregate influence of all active propaganda at a location
    /// when a newcomer arrives.
    ///
    /// Groups influence by `associated_construct` (or by content if no
    /// construct is linked). Returns a list of `PropagandaInfluence` entries,
    /// one per group.
    pub fn influence_on_newcomer(
        &self,
        location: LocationId,
    ) -> Vec<PropagandaInfluence> {
        let active_posts = self.get_posts_at_location(location);

        if active_posts.is_empty() {
            return Vec::new();
        }

        // Group by (associated_construct, propaganda_type)
        // Use a composite key for grouping
        let mut groups: BTreeMap<(Option<Uuid>, PropagandaType), (f64, u32, String)> =
            BTreeMap::new();

        for post in &active_posts {
            let key = (post.associated_construct, post.propaganda_type);
            let entry = groups.entry(key).or_insert((0.0, 0, String::new()));
            entry.0 += post.influence_strength;
            entry.1 = entry.1.saturating_add(1);
            // Use the first post's content as the summary
            if entry.2.is_empty() {
                entry.2.clone_from(&post.content);
            }
        }

        groups
            .into_iter()
            .map(|((construct, prop_type), (total_influence, count, summary))| {
                PropagandaInfluence {
                    source_type: prop_type,
                    content_summary: summary,
                    total_influence,
                    associated_construct: construct,
                    post_count: count,
                }
            })
            .collect()
    }

    /// Post counter-propaganda at the same location to weaken existing posts.
    ///
    /// Finds all active posts at the location that match the opposing criteria
    /// (same construct or same propaganda type) and reduces their influence
    /// strength. Also creates a new counter-propaganda post.
    ///
    /// Returns the ID of the new counter-propaganda post and the number
    /// of posts weakened.
    pub fn counter_propaganda(&mut self, params: &PropagandaParams) -> (Uuid, u32) {
        // First, weaken existing posts at this location with the same construct
        // or the same propaganda type (but different author).
        let post_ids: Vec<Uuid> = self
            .location_index
            .get(&params.location)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default();

        let mut weakened_count: u32 = 0;

        for post_id in &post_ids {
            if let Some(post) = self.posts.get_mut(post_id) {
                if post.expired || post.author == params.author {
                    continue;
                }

                let matches_construct = params.associated_construct.is_some()
                    && post.associated_construct == params.associated_construct;
                let matches_type = post.propaganda_type == params.propaganda_type;

                if matches_construct || matches_type {
                    post.influence_strength =
                        clamp_influence(post.influence_strength - COUNTER_PROPAGANDA_PENALTY);
                    weakened_count = weakened_count.saturating_add(1);
                }
            }
        }

        // Then post the counter-propaganda
        let new_id = self.post_propaganda(params);

        (new_id, weakened_count)
    }

    /// Get locations ranked by total active post count.
    ///
    /// Returns a list of (location, `active_post_count`) sorted descending.
    pub fn most_propagandized_locations(&self) -> Vec<(LocationId, u32)> {
        let mut location_counts: BTreeMap<LocationId, u32> = BTreeMap::new();

        for post in self.posts.values() {
            if !post.expired {
                let count = location_counts.entry(post.location).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        let mut results: Vec<(LocationId, u32)> = location_counts.into_iter().collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }

    /// Record that an agent visited a location at a given tick.
    ///
    /// Used for reach estimation.
    pub fn record_visit(
        &mut self,
        location: LocationId,
        agent_id: AgentId,
        tick: u64,
    ) {
        let loc_visits = self.visit_log.entry(location).or_default();
        // Only record the first visit (earliest tick)
        loc_visits.entry(agent_id).or_insert(tick);
    }

    /// Estimate how many agents have been exposed to a specific post.
    ///
    /// Counts agents who visited the post's location at or after the
    /// tick the post was created.
    pub fn propaganda_reach(&self, post_id: Uuid) -> u32 {
        let Some(post) = self.posts.get(&post_id) else {
            return 0;
        };

        self.visit_log
            .get(&post.location)
            .map_or(0, |visits| {
                let count = visits
                    .values()
                    .filter(|&&visit_tick| visit_tick >= post.tick_posted)
                    .count();
                // Saturating conversion from usize to u32
                u32::try_from(count).unwrap_or(u32::MAX)
            })
    }
}

impl Default for PropagandaBoard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Clamp influence strength to the valid range [0.0, 1.0].
const fn clamp_influence(value: f64) -> f64 {
    value.clamp(MIN_INFLUENCE, MAX_INFLUENCE)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use emergence_types::{AgentId, LocationId};
    use uuid::Uuid;

    use super::*;

    // -----------------------------------------------------------------------
    // Helper to build params concisely
    // -----------------------------------------------------------------------

    fn params(
        author: AgentId,
        location: LocationId,
        tick: u64,
        propaganda_type: PropagandaType,
        content: &str,
        associated_construct: Option<Uuid>,
        influence_strength: f64,
    ) -> PropagandaParams {
        PropagandaParams {
            author,
            location,
            tick,
            propaganda_type,
            content: String::from(content),
            associated_construct,
            influence_strength,
        }
    }

    // -----------------------------------------------------------------------
    // Basic posting and retrieval
    // -----------------------------------------------------------------------

    #[test]
    fn new_board_is_empty() {
        let board = PropagandaBoard::new();
        let location = LocationId::new();
        assert!(board.get_posts_at_location(location).is_empty());
    }

    #[test]
    fn post_and_retrieve_at_location() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let p = params(author, location, 10, PropagandaType::Declaration, "No stealing allowed", None, 0.7);
        let id = board.post_propaganda(&p);

        let posts = board.get_posts_at_location(location);
        assert_eq!(posts.len(), 1);
        assert_eq!(posts.first().map(|p| p.id), Some(id));
        assert_eq!(
            posts.first().map(|p| &p.content),
            Some(&String::from("No stealing allowed"))
        );
    }

    #[test]
    fn retrieve_by_author() {
        let mut board = PropagandaBoard::new();
        let author_a = AgentId::new();
        let author_b = AgentId::new();
        let location = LocationId::new();

        let _ = board.post_propaganda(&params(author_a, location, 10, PropagandaType::Declaration, "Post A", None, 0.5));
        let _ = board.post_propaganda(&params(author_b, location, 20, PropagandaType::Recruitment, "Post B", None, 0.6));

        let posts_a = board.get_posts_by_author(author_a);
        assert_eq!(posts_a.len(), 1);
        assert_eq!(posts_a.first().map(|p| p.author), Some(author_a));

        let posts_b = board.get_posts_by_author(author_b);
        assert_eq!(posts_b.len(), 1);
        assert_eq!(posts_b.first().map(|p| p.author), Some(author_b));
    }

    #[test]
    fn retrieve_by_type() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let _ = board.post_propaganda(&params(author, location, 10, PropagandaType::Declaration, "Rule", None, 0.5));
        let _ = board.post_propaganda(&params(author, location, 20, PropagandaType::Recruitment, "Join us", None, 0.6));
        let _ = board.post_propaganda(&params(author, location, 30, PropagandaType::Declaration, "Another rule", None, 0.4));

        let declarations = board.get_posts_by_type(PropagandaType::Declaration);
        assert_eq!(declarations.len(), 2);

        let recruitment = board.get_posts_by_type(PropagandaType::Recruitment);
        assert_eq!(recruitment.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Construct-linked filtering
    // -----------------------------------------------------------------------

    #[test]
    fn retrieve_by_construct() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();
        let construct_id = Uuid::now_v7();

        let _ = board.post_propaganda(&params(author, location, 10, PropagandaType::Doctrine, "The river provides", Some(construct_id), 0.7));
        let _ = board.post_propaganda(&params(author, location, 20, PropagandaType::Declaration, "No construct", None, 0.5));

        let construct_posts = board.get_posts_for_construct(construct_id);
        assert_eq!(construct_posts.len(), 1);
        assert_eq!(
            construct_posts.first().map(|p| p.associated_construct),
            Some(Some(construct_id))
        );
    }

    // -----------------------------------------------------------------------
    // Expiration (manual + auto)
    // -----------------------------------------------------------------------

    #[test]
    fn manual_expire_post() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let id = board.post_propaganda(&params(author, location, 10, PropagandaType::Warning, "Beware", None, 0.5));

        assert_eq!(board.get_posts_at_location(location).len(), 1);

        let result = board.expire_post(id);
        assert!(result.is_ok());

        // Active posts should be empty after expiration
        assert!(board.get_posts_at_location(location).is_empty());
    }

    #[test]
    fn manual_expire_nonexistent_post() {
        let mut board = PropagandaBoard::new();
        let result = board.expire_post(Uuid::now_v7());
        assert!(result.is_err());
    }

    #[test]
    fn auto_expire_old_posts() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        // Post at tick 10
        let _ = board.post_propaganda(&params(author, location, 10, PropagandaType::Declaration, "Old post", None, 0.5));
        // Post at tick 400
        let _ = board.post_propaganda(&params(author, location, 400, PropagandaType::Declaration, "Recent post", None, 0.5));

        // Auto-expire at tick 600 with default 500 ticks
        // Threshold = 600 - 500 = 100; post at tick 10 < 100 -> expired
        let expired_count = board.auto_expire(600, None);
        assert_eq!(expired_count, 1);

        let active = board.get_posts_at_location(location);
        assert_eq!(active.len(), 1);
        assert_eq!(
            active.first().map(|p| &p.content),
            Some(&String::from("Recent post"))
        );
    }

    #[test]
    fn auto_expire_custom_age() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let _ = board.post_propaganda(&params(author, location, 10, PropagandaType::History, "Ancient history", None, 0.5));

        // Custom max age of 50 ticks, current tick 100
        // Threshold = 100 - 50 = 50; post at tick 10 < 50 -> expired
        let expired_count = board.auto_expire(100, Some(50));
        assert_eq!(expired_count, 1);
        assert!(board.get_posts_at_location(location).is_empty());
    }

    // -----------------------------------------------------------------------
    // Newcomer influence calculation
    // -----------------------------------------------------------------------

    #[test]
    fn newcomer_influence_empty_location() {
        let board = PropagandaBoard::new();
        let location = LocationId::new();
        let influence = board.influence_on_newcomer(location);
        assert!(influence.is_empty());
    }

    #[test]
    fn newcomer_influence_aggregates_by_construct_and_type() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();
        let construct_id = Uuid::now_v7();

        // Two posts for the same construct + type
        let _ = board.post_propaganda(&params(author, location, 10, PropagandaType::Doctrine, "Believe", Some(construct_id), 0.5));
        let _ = board.post_propaganda(&params(author, location, 20, PropagandaType::Doctrine, "Believe more", Some(construct_id), 0.3));

        // One post for a different type
        let _ = board.post_propaganda(&params(author, location, 30, PropagandaType::Warning, "Beware", None, 0.4));

        let influence = board.influence_on_newcomer(location);
        assert_eq!(influence.len(), 2);

        // Find the doctrine influence
        let doctrine = influence
            .iter()
            .find(|i| i.source_type == PropagandaType::Doctrine);
        assert!(doctrine.is_some());
        let fallback = PropagandaInfluence {
            source_type: PropagandaType::Declaration,
            content_summary: String::new(),
            total_influence: 0.0,
            associated_construct: None,
            post_count: 0,
        };
        let d = doctrine.unwrap_or(&fallback);
        assert!((d.total_influence - 0.8).abs() < f64::EPSILON);
        assert_eq!(d.post_count, 2);
        assert_eq!(d.associated_construct, Some(construct_id));
    }

    // -----------------------------------------------------------------------
    // Counter-propaganda
    // -----------------------------------------------------------------------

    #[test]
    fn counter_propaganda_weakens_existing() {
        let mut board = PropagandaBoard::new();
        let author_a = AgentId::new();
        let author_b = AgentId::new();
        let location = LocationId::new();
        let construct_id = Uuid::now_v7();

        // Original post with influence 0.5
        let original_id = board.post_propaganda(&params(author_a, location, 10, PropagandaType::Doctrine, "Original doctrine", Some(construct_id), 0.5));

        // Counter-propaganda by a different author
        let counter_params = params(author_b, location, 20, PropagandaType::Doctrine, "Counter doctrine", Some(construct_id), 0.6);
        let (counter_id, weakened) = board.counter_propaganda(&counter_params);

        assert_eq!(weakened, 1);
        assert_ne!(counter_id, original_id);

        // Check the original post's influence was reduced
        let original = board.posts.get(&original_id);
        assert!(original.is_some());
        let orig_influence = original.map(|p| p.influence_strength).unwrap_or(1.0);
        // 0.5 - 0.15 (penalty) = 0.35
        assert!((orig_influence - 0.35).abs() < f64::EPSILON);

        // Counter post should exist with full influence
        let counter = board.posts.get(&counter_id);
        assert!(counter.is_some());
        let counter_influence = counter.map(|p| p.influence_strength).unwrap_or(0.0);
        assert!((counter_influence - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_propaganda_does_not_weaken_own_posts() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        // Original post
        let original_id = board.post_propaganda(&params(author, location, 10, PropagandaType::Declaration, "My rule", None, 0.5));

        // Counter-propaganda by the SAME author
        let counter_params = params(author, location, 20, PropagandaType::Declaration, "My updated rule", None, 0.6);
        let (_, weakened) = board.counter_propaganda(&counter_params);

        // Should not weaken own posts
        assert_eq!(weakened, 0);

        let original = board.posts.get(&original_id);
        let orig_influence = original.map(|p| p.influence_strength).unwrap_or(0.0);
        assert!((orig_influence - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_propaganda_influence_floors_at_zero() {
        let mut board = PropagandaBoard::new();
        let author_a = AgentId::new();
        let author_b = AgentId::new();
        let location = LocationId::new();

        // Original post with very low influence
        let original_id = board.post_propaganda(&params(author_a, location, 10, PropagandaType::Warning, "Weak warning", None, 0.05));

        // Counter-propaganda should not reduce below 0.0
        let counter_params = params(author_b, location, 20, PropagandaType::Warning, "Counter warning", None, 0.5);
        let (_, weakened) = board.counter_propaganda(&counter_params);

        assert_eq!(weakened, 1);

        let original = board.posts.get(&original_id);
        let orig_influence = original.map(|p| p.influence_strength).unwrap_or(1.0);
        assert!(orig_influence >= 0.0);
        assert!(orig_influence.abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Most propagandized locations
    // -----------------------------------------------------------------------

    #[test]
    fn most_propagandized_locations_sorted() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let loc_a = LocationId::new();
        let loc_b = LocationId::new();
        let loc_c = LocationId::new();

        // 3 posts at loc_a
        for _ in 0..3 {
            let _ = board.post_propaganda(&params(author, loc_a, 10, PropagandaType::Declaration, "Rule", None, 0.5));
        }
        // 1 post at loc_b
        let _ = board.post_propaganda(&params(author, loc_b, 10, PropagandaType::Warning, "Warning", None, 0.5));
        // 2 posts at loc_c
        for _ in 0..2 {
            let _ = board.post_propaganda(&params(author, loc_c, 10, PropagandaType::Recruitment, "Join", None, 0.5));
        }

        let ranked = board.most_propagandized_locations();
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked.first().map(|(_, count)| *count), Some(3));
        assert_eq!(ranked.first().map(|(loc, _)| *loc), Some(loc_a));
        assert_eq!(ranked.get(1).map(|(_, count)| *count), Some(2));
        assert_eq!(ranked.last().map(|(_, count)| *count), Some(1));
    }

    // -----------------------------------------------------------------------
    // Reach tracking
    // -----------------------------------------------------------------------

    #[test]
    fn reach_counts_visitors_after_posting() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();
        let visitor_a = AgentId::new();
        let visitor_b = AgentId::new();
        let visitor_c = AgentId::new();

        // Post at tick 10
        let post_id = board.post_propaganda(&params(author, location, 10, PropagandaType::Declaration, "Rule", None, 0.5));

        // Visitor A came before the post (tick 5)
        board.record_visit(location, visitor_a, 5);
        // Visitor B came after (tick 15)
        board.record_visit(location, visitor_b, 15);
        // Visitor C came after (tick 20)
        board.record_visit(location, visitor_c, 20);

        let reach = board.propaganda_reach(post_id);
        // Only B and C visited after tick 10
        assert_eq!(reach, 2);
    }

    #[test]
    fn reach_for_nonexistent_post() {
        let board = PropagandaBoard::new();
        assert_eq!(board.propaganda_reach(Uuid::now_v7()), 0);
    }

    #[test]
    fn reach_includes_exact_tick() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();
        let visitor = AgentId::new();

        let post_id = board.post_propaganda(&params(author, location, 10, PropagandaType::Declaration, "Rule", None, 0.5));

        // Visitor arrived exactly at post tick
        board.record_visit(location, visitor, 10);

        let reach = board.propaganda_reach(post_id);
        assert_eq!(reach, 1);
    }

    // -----------------------------------------------------------------------
    // Influence clamping
    // -----------------------------------------------------------------------

    #[test]
    fn influence_clamped_to_max() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let id = board.post_propaganda(&params(author, location, 10, PropagandaType::Tribute, "Glory!", None, 1.5));

        let post = board.posts.get(&id);
        assert!(post.is_some());
        assert!(
            (post.map(|p| p.influence_strength).unwrap_or(0.0) - 1.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn influence_clamped_to_min() {
        let mut board = PropagandaBoard::new();
        let author = AgentId::new();
        let location = LocationId::new();

        let id = board.post_propaganda(&params(author, location, 10, PropagandaType::Tribute, "Shame!", None, -0.5));

        let post = board.posts.get(&id);
        assert!(post.is_some());
        assert!(
            post.map(|p| p.influence_strength).unwrap_or(1.0).abs() < f64::EPSILON
        );
    }

    // -----------------------------------------------------------------------
    // Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn default_board_is_empty() {
        let board = PropagandaBoard::default();
        assert!(board.most_propagandized_locations().is_empty());
    }
}
