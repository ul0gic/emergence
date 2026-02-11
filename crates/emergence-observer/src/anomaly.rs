//! Anomaly detection layer for emergent behavioral pattern analysis.
//!
//! Implements Phase 8.3: unsupervised clustering of agent behavior vectors
//! to detect novel social structures and flag statistical outliers without
//! imposing human-category labels.
//!
//! # Architecture
//!
//! - [`BehaviorVector`]: action frequency, interaction pattern, and resource
//!   delta counters computed over a configurable tick window
//! - [`BehaviorCluster`]: k-means cluster result with centroid, member list,
//!   and creation tick
//! - [`AnomalyFlag`]: agents whose behavior vectors are statistical outliers
//!   (distance from nearest cluster centroid > 2 standard deviations)
//! - [`AnomalyState`]: in-memory store updated each tick, served by REST
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET` | `/api/anomalies/clusters` | Current behavior clusters |
//! | `GET` | `/api/anomalies/flags` | Flagged anomalous agents |

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use emergence_types::AgentId;

use crate::error::ObserverError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default number of ticks in the behavior analysis window.
const DEFAULT_WINDOW_SIZE: u64 = 50;

/// Default number of clusters for k-means.
const DEFAULT_K: usize = 3;

/// Maximum number of k-means iterations before convergence is declared.
const MAX_KMEANS_ITERATIONS: usize = 50;

/// Outlier threshold: distance from nearest centroid beyond this many
/// standard deviations is flagged as anomalous.
const OUTLIER_SIGMA: f64 = 2.0;

// ---------------------------------------------------------------------------
// Behavior Vector
// ---------------------------------------------------------------------------

/// A category-free behavioral pattern vector for a single agent computed
/// over a tick window.
///
/// Action type counts, interaction partner counts, and net resource deltas
/// are encoded as parallel dimension vectors so that Euclidean distance
/// is meaningful for clustering.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorVector {
    /// The agent this vector describes.
    pub agent_id: AgentId,
    /// Start tick of the observation window (inclusive).
    pub window_start: u64,
    /// End tick of the observation window (inclusive).
    pub window_end: u64,
    /// Action type frequency counts within the window.
    pub action_counts: BTreeMap<String, u32>,
    /// Interaction partner frequency counts (agent ID -> count).
    pub interaction_counts: BTreeMap<String, u32>,
    /// Net resource delta map (resource name -> signed change).
    pub resource_deltas: BTreeMap<String, i64>,
}

impl BehaviorVector {
    /// Convert this vector into a flat f64 slice for distance computation.
    ///
    /// Dimensions are: all action counts (sorted by key), then interaction
    /// counts (sorted by key), then resource deltas (sorted by key).
    fn to_dimensions(&self, action_keys: &[String], interaction_keys: &[String], resource_keys: &[String]) -> Vec<f64> {
        let mut dims = Vec::with_capacity(
            action_keys.len()
                .saturating_add(interaction_keys.len())
                .saturating_add(resource_keys.len()),
        );

        for key in action_keys {
            let count = self.action_counts.get(key).copied().unwrap_or(0);
            dims.push(f64::from(count));
        }
        for key in interaction_keys {
            let count = self.interaction_counts.get(key).copied().unwrap_or(0);
            dims.push(f64::from(count));
        }
        for key in resource_keys {
            let delta = self.resource_deltas.get(key).copied().unwrap_or(0);
            #[allow(clippy::cast_precision_loss)]
            dims.push(delta as f64);
        }

        dims
    }
}

// ---------------------------------------------------------------------------
// Cluster
// ---------------------------------------------------------------------------

/// A cluster of agents with similar behavioral patterns.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorCluster {
    /// Unique cluster identifier (0-indexed).
    pub cluster_id: u32,
    /// Agent IDs assigned to this cluster.
    pub member_agent_ids: Vec<AgentId>,
    /// Centroid of the cluster (same dimensionality as behavior vectors).
    pub centroid: Vec<f64>,
    /// Tick when this cluster was computed.
    pub creation_tick: u64,
    /// Number of members.
    pub member_count: u32,
}

// ---------------------------------------------------------------------------
// Anomaly Flag
// ---------------------------------------------------------------------------

/// An anomaly flag for an agent whose behavior is a statistical outlier.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnomalyFlag {
    /// The flagged agent.
    pub agent_id: AgentId,
    /// Tick when the flag was raised.
    pub tick: u64,
    /// The behavior vector that triggered the flag.
    pub behavior_vector: BehaviorVector,
    /// Euclidean distance from the nearest cluster centroid.
    pub distance_from_nearest_cluster: f64,
    /// The nearest cluster ID.
    pub nearest_cluster_id: u32,
    /// Human-readable description of the anomaly.
    pub description: String,
}

// ---------------------------------------------------------------------------
// Anomaly State (in-memory store)
// ---------------------------------------------------------------------------

/// In-memory anomaly detection state updated periodically.
#[derive(Debug, Clone, Default)]
pub struct AnomalyState {
    /// Current behavior vectors for all tracked agents.
    pub behavior_vectors: Vec<BehaviorVector>,
    /// Current cluster assignments.
    pub clusters: Vec<BehaviorCluster>,
    /// Current anomaly flags.
    pub flags: Vec<AnomalyFlag>,
    /// Last tick when clustering was performed.
    pub last_analysis_tick: u64,
}

// ---------------------------------------------------------------------------
// Behavior Vector Computation
// ---------------------------------------------------------------------------

/// Compute behavior vectors for all alive agents from the event log.
///
/// This scans events within the window `[current_tick - window_size, current_tick]`
/// and tallies action frequencies, interaction partners, and resource deltas
/// per agent.
pub fn compute_behavior_vectors(
    snapshot: &crate::state::SimulationSnapshot,
    window_size: u64,
) -> Vec<BehaviorVector> {
    let current_tick = snapshot.current_tick;
    let window_start = current_tick.saturating_sub(window_size);

    // Build per-agent accumulators.
    let mut accumulators: BTreeMap<AgentId, BehaviorVectorAccumulator> = BTreeMap::new();

    // Initialize accumulators for all alive agents.
    for agent_state in snapshot.agent_states.values() {
        if snapshot
            .agents
            .get(&agent_state.agent_id)
            .is_some_and(|a| a.died_at_tick.is_none())
        {
            accumulators
                .entry(agent_state.agent_id)
                .or_insert_with(|| BehaviorVectorAccumulator {
                    action_counts: BTreeMap::new(),
                    interaction_counts: BTreeMap::new(),
                    resource_deltas: BTreeMap::new(),
                });
        }
    }

    // Scan events within the window.
    for event in &snapshot.events {
        if event.tick < window_start || event.tick > current_tick {
            continue;
        }

        let Some(agent_id) = event.agent_id else {
            continue;
        };

        let Some(acc) = accumulators.get_mut(&agent_id) else {
            continue;
        };

        // Tally action type.
        let action_type_str = format!("{:?}", event.event_type);
        let entry = acc.action_counts.entry(action_type_str).or_insert(0u32);
        *entry = entry.saturating_add(1);

        // Extract interaction partners from event details.
        extract_interaction_partners(event, agent_id, acc);

        // Extract resource deltas from event details.
        extract_resource_deltas(event, acc);
    }

    // Convert accumulators to behavior vectors.
    accumulators
        .into_iter()
        .map(|(agent_id, acc)| BehaviorVector {
            agent_id,
            window_start,
            window_end: current_tick,
            action_counts: acc.action_counts,
            interaction_counts: acc.interaction_counts,
            resource_deltas: acc.resource_deltas,
        })
        .collect()
}

/// Internal accumulator for building a behavior vector.
struct BehaviorVectorAccumulator {
    action_counts: BTreeMap<String, u32>,
    interaction_counts: BTreeMap<String, u32>,
    resource_deltas: BTreeMap<String, i64>,
}

/// Extract interaction partner IDs from event details.
fn extract_interaction_partners(
    event: &emergence_types::Event,
    agent_id: AgentId,
    acc: &mut BehaviorVectorAccumulator,
) {
    let Some(details) = event.details.as_object() else {
        return;
    };

    // Look for common partner fields in event details.
    let partner_fields = [
        "agent_a", "agent_b", "target_agent", "teacher_id", "student_id",
        "thief_id", "victim_id", "attacker_id", "defender_id", "offerer_id",
        "target_id",
    ];

    for field in &partner_fields {
        if let Some(partner_val) = details.get(*field)
            && let Some(partner_str) = partner_val.as_str()
        {
            // Only record the partner, not ourselves.
            let self_str = agent_id.to_string();
            if partner_str != self_str {
                let entry = acc
                    .interaction_counts
                    .entry(partner_str.to_owned())
                    .or_insert(0u32);
                *entry = entry.saturating_add(1);
            }
        }
    }
}

/// Extract resource deltas from event details.
fn extract_resource_deltas(
    event: &emergence_types::Event,
    acc: &mut BehaviorVectorAccumulator,
) {
    let Some(details) = event.details.as_object() else {
        return;
    };

    // Look for resource and quantity fields.
    if let Some(resource_val) = details.get("resource")
        && let Some(resource_str) = resource_val.as_str()
        && let Some(quantity_val) = details.get("quantity")
    {
        let quantity = quantity_val.as_i64().unwrap_or(0);
        let is_consumption = matches!(
            event.event_type,
            emergence_types::EventType::ResourceConsumed
        );
        let delta = if is_consumption {
            quantity.saturating_neg()
        } else {
            quantity
        };

        let entry = acc
            .resource_deltas
            .entry(resource_str.to_owned())
            .or_insert(0i64);
        *entry = entry.saturating_add(delta);
    }

    // Check for gave/received maps in trade events.
    if let Some(gave) = details.get("gave")
        && let Some(gave_obj) = gave.as_object()
    {
        for (resource, qty) in gave_obj {
            let delta = qty.as_i64().unwrap_or(0).saturating_neg();
            let entry = acc
                .resource_deltas
                .entry(resource.clone())
                .or_insert(0i64);
            *entry = entry.saturating_add(delta);
        }
    }
    if let Some(received) = details.get("received")
        && let Some(received_obj) = received.as_object()
    {
        for (resource, qty) in received_obj {
            let delta = qty.as_i64().unwrap_or(0);
            let entry = acc
                .resource_deltas
                .entry(resource.clone())
                .or_insert(0i64);
            *entry = entry.saturating_add(delta);
        }
    }
}

// ---------------------------------------------------------------------------
// K-Means Clustering
// ---------------------------------------------------------------------------

/// Build a unified dimension key set from all behavior vectors.
fn build_dimension_keys(vectors: &[BehaviorVector]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut action_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut interaction_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut resource_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for v in vectors {
        for key in v.action_counts.keys() {
            action_keys.insert(key.clone());
        }
        for key in v.interaction_counts.keys() {
            interaction_keys.insert(key.clone());
        }
        for key in v.resource_deltas.keys() {
            resource_keys.insert(key.clone());
        }
    }

    (
        action_keys.into_iter().collect(),
        interaction_keys.into_iter().collect(),
        resource_keys.into_iter().collect(),
    )
}

/// Run k-means clustering on behavior vectors.
///
/// Returns cluster assignments. If there are fewer vectors than `k`, each
/// vector becomes its own cluster.
#[allow(clippy::too_many_lines)]
pub fn cluster_behavior_vectors(
    vectors: &[BehaviorVector],
    k: usize,
    current_tick: u64,
) -> Vec<BehaviorCluster> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let effective_k = k.min(vectors.len());
    let (action_keys, interaction_keys, resource_keys) = build_dimension_keys(vectors);

    // Convert all vectors to flat f64 arrays.
    let data: Vec<Vec<f64>> = vectors
        .iter()
        .map(|v| v.to_dimensions(&action_keys, &interaction_keys, &resource_keys))
        .collect();

    let dim_count = data.first().map_or(0, Vec::len);
    if dim_count == 0 {
        // All vectors are empty -- assign everyone to cluster 0.
        return vec![BehaviorCluster {
            cluster_id: 0,
            member_agent_ids: vectors.iter().map(|v| v.agent_id).collect(),
            centroid: Vec::new(),
            creation_tick: current_tick,
            #[allow(clippy::cast_possible_truncation)]
            member_count: vectors.len() as u32,
        }];
    }

    // Initialize centroids: pick evenly-spaced data points to avoid
    // clustering failure when similar points are adjacent in the input.
    let mut centroids: Vec<Vec<f64>> = Vec::with_capacity(effective_k);
    let data_len = data.len();
    for i in 0..effective_k {
        // Spread initial centroids across the data set.
        #[allow(clippy::arithmetic_side_effects)]
        let idx = if effective_k <= 1 {
            0
        } else {
            (i.saturating_mul(data_len.saturating_sub(1)))
                / effective_k.saturating_sub(1).max(1)
        };
        if let Some(point) = data.get(idx) {
            centroids.push(point.clone());
        }
    }

    // Assignment vector: which cluster each data point belongs to.
    let mut assignments: Vec<usize> = vec![0; data.len()];

    for _iteration in 0..MAX_KMEANS_ITERATIONS {
        let mut changed = false;

        // Assignment step: assign each point to the nearest centroid.
        for (i, point) in data.iter().enumerate() {
            let mut best_cluster = 0usize;
            let mut best_dist = f64::MAX;

            for (c, centroid) in centroids.iter().enumerate() {
                let dist = euclidean_distance(point, centroid);
                if dist < best_dist {
                    best_dist = dist;
                    best_cluster = c;
                }
            }

            if let Some(prev) = assignments.get(i)
                && *prev != best_cluster
            {
                changed = true;
            }
            if let Some(slot) = assignments.get_mut(i) {
                *slot = best_cluster;
            }
        }

        if !changed {
            break;
        }

        // Update step: recompute centroids as mean of assigned points.
        for (c, centroid) in centroids.iter_mut().enumerate() {
            let mut sum = vec![0.0_f64; dim_count];
            let mut count = 0u64;

            for (i, point) in data.iter().enumerate() {
                if assignments.get(i).copied() == Some(c) {
                    for (d, val) in point.iter().enumerate() {
                        if let Some(s) = sum.get_mut(d) {
                            *s += val;
                        }
                    }
                    count = count.saturating_add(1);
                }
            }

            if count > 0 {
                #[allow(clippy::cast_precision_loss)]
                let count_f = count as f64;
                for s in &mut sum {
                    *s /= count_f;
                }
                *centroid = sum;
            }
        }
    }

    // Build cluster results.
    let mut clusters: Vec<BehaviorCluster> = Vec::with_capacity(effective_k);
    for c in 0..effective_k {
        let members: Vec<AgentId> = assignments
            .iter()
            .enumerate()
            .filter(|(_, cluster)| **cluster == c)
            .filter_map(|(i, _)| vectors.get(i).map(|v| v.agent_id))
            .collect();

        #[allow(clippy::cast_possible_truncation)]
        let member_count = members.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let cluster_id = c as u32;

        clusters.push(BehaviorCluster {
            cluster_id,
            member_agent_ids: members,
            centroid: centroids.get(c).cloned().unwrap_or_default(),
            creation_tick: current_tick,
            member_count,
        });
    }

    // Remove empty clusters.
    clusters.retain(|c| c.member_count > 0);

    clusters
}

/// Compute Euclidean distance between two points.
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    let len = a.len().min(b.len());
    for i in 0..len {
        let diff = a.get(i).copied().unwrap_or(0.0) - b.get(i).copied().unwrap_or(0.0);
        sum += diff * diff;
    }
    sum.sqrt()
}

// ---------------------------------------------------------------------------
// Anomaly Flagging
// ---------------------------------------------------------------------------

/// Flag agents whose behavior vectors are statistical outliers.
///
/// An agent is flagged if its Euclidean distance from the nearest cluster
/// centroid exceeds `mean_distance + OUTLIER_SIGMA * std_dev`.
pub fn flag_anomalies(
    vectors: &[BehaviorVector],
    clusters: &[BehaviorCluster],
    current_tick: u64,
) -> Vec<AnomalyFlag> {
    if vectors.is_empty() || clusters.is_empty() {
        return Vec::new();
    }

    let (action_keys, interaction_keys, resource_keys) = build_dimension_keys(vectors);

    // Compute distance from each vector to its nearest centroid.
    let mut distances: Vec<(usize, f64, u32)> = Vec::with_capacity(vectors.len());
    for (i, v) in vectors.iter().enumerate() {
        let point = v.to_dimensions(&action_keys, &interaction_keys, &resource_keys);
        let mut best_dist = f64::MAX;
        let mut best_cluster_id = 0u32;

        for cluster in clusters {
            let dist = euclidean_distance(&point, &cluster.centroid);
            if dist < best_dist {
                best_dist = dist;
                best_cluster_id = cluster.cluster_id;
            }
        }

        distances.push((i, best_dist, best_cluster_id));
    }

    // Compute mean and standard deviation of distances.
    let count = distances.len();
    if count == 0 {
        return Vec::new();
    }

    #[allow(clippy::cast_precision_loss)]
    let count_f = count as f64;
    let sum: f64 = distances.iter().map(|(_, d, _)| d).sum();
    let mean = sum / count_f;

    let variance_sum: f64 = distances
        .iter()
        .map(|(_, d, _)| {
            let diff = d - mean;
            diff * diff
        })
        .sum();
    let std_dev = (variance_sum / count_f).sqrt();

    let threshold = OUTLIER_SIGMA.mul_add(std_dev, mean);

    // Flag agents exceeding the threshold.
    let mut flags: Vec<AnomalyFlag> = Vec::new();
    for (i, dist, cluster_id) in &distances {
        if *dist > threshold
            && let Some(v) = vectors.get(*i)
        {
            flags.push(AnomalyFlag {
                agent_id: v.agent_id,
                tick: current_tick,
                behavior_vector: v.clone(),
                distance_from_nearest_cluster: *dist,
                nearest_cluster_id: *cluster_id,
                description: format!(
                    "Agent {} behavior is {:.2} standard deviations from nearest cluster (distance: {:.2}, threshold: {:.2})",
                    v.agent_id,
                    if std_dev > 0.0 { (dist - mean) / std_dev } else { 0.0 },
                    dist,
                    threshold,
                ),
            });
        }
    }

    flags
}

// ---------------------------------------------------------------------------
// Full Analysis Pipeline
// ---------------------------------------------------------------------------

/// Run the full anomaly detection pipeline: compute vectors, cluster, flag.
///
/// Returns the updated anomaly state.
pub fn run_anomaly_analysis(
    snapshot: &crate::state::SimulationSnapshot,
) -> AnomalyState {
    let current_tick = snapshot.current_tick;
    let vectors = compute_behavior_vectors(snapshot, DEFAULT_WINDOW_SIZE);
    let clusters = cluster_behavior_vectors(&vectors, DEFAULT_K, current_tick);
    let flags = flag_anomalies(&vectors, &clusters, current_tick);

    AnomalyState {
        behavior_vectors: vectors,
        clusters,
        flags,
        last_analysis_tick: current_tick,
    }
}

// ---------------------------------------------------------------------------
// REST Handlers
// ---------------------------------------------------------------------------

/// `GET /api/anomalies/clusters` -- return current behavior clusters.
pub async fn get_clusters(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    // Run analysis on demand from the current snapshot.
    let analysis = run_anomaly_analysis(&snapshot);

    Ok(Json(serde_json::json!({
        "tick": snapshot.current_tick,
        "cluster_count": analysis.clusters.len(),
        "clusters": analysis.clusters,
        "behavior_vector_count": analysis.behavior_vectors.len(),
    })))
}

/// `GET /api/anomalies/flags` -- return current anomaly flags.
pub async fn get_flags(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    // Run analysis on demand from the current snapshot.
    let analysis = run_anomaly_analysis(&snapshot);

    Ok(Json(serde_json::json!({
        "tick": snapshot.current_tick,
        "flag_count": analysis.flags.len(),
        "flags": analysis.flags,
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euclidean_distance_basic() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 1e-10);
    }

    #[test]
    fn euclidean_distance_same_point() {
        let a = vec![1.0, 2.0, 3.0];
        let dist = euclidean_distance(&a, &a);
        assert!((dist - 0.0).abs() < 1e-10);
    }

    #[test]
    fn cluster_empty_vectors() {
        let clusters = cluster_behavior_vectors(&[], 3, 100);
        assert!(clusters.is_empty());
    }

    #[test]
    fn cluster_single_vector() {
        let vectors = vec![BehaviorVector {
            agent_id: AgentId::new(),
            window_start: 0,
            window_end: 50,
            action_counts: {
                let mut m = BTreeMap::new();
                m.insert("Gather".to_owned(), 10);
                m
            },
            interaction_counts: BTreeMap::new(),
            resource_deltas: BTreeMap::new(),
        }];

        let clusters = cluster_behavior_vectors(&vectors, 3, 50);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters.first().map(|c| c.member_count), Some(1));
    }

    #[test]
    fn cluster_two_distinct_groups() {
        // Group A: gatherers
        let mut group_a: Vec<BehaviorVector> = Vec::new();
        for _ in 0..5 {
            group_a.push(BehaviorVector {
                agent_id: AgentId::new(),
                window_start: 0,
                window_end: 50,
                action_counts: {
                    let mut m = BTreeMap::new();
                    m.insert("Gather".to_owned(), 100);
                    m.insert("Rest".to_owned(), 5);
                    m
                },
                interaction_counts: BTreeMap::new(),
                resource_deltas: BTreeMap::new(),
            });
        }

        // Group B: socializers
        let mut group_b: Vec<BehaviorVector> = Vec::new();
        for _ in 0..5 {
            group_b.push(BehaviorVector {
                agent_id: AgentId::new(),
                window_start: 0,
                window_end: 50,
                action_counts: {
                    let mut m = BTreeMap::new();
                    m.insert("Gather".to_owned(), 5);
                    m.insert("Communicate".to_owned(), 100);
                    m
                },
                interaction_counts: BTreeMap::new(),
                resource_deltas: BTreeMap::new(),
            });
        }

        let mut all_vectors = group_a;
        all_vectors.extend(group_b);

        let clusters = cluster_behavior_vectors(&all_vectors, 2, 50);
        // Should produce 2 clusters with 5 members each.
        assert_eq!(clusters.len(), 2);
        for c in &clusters {
            assert_eq!(c.member_count, 5);
        }
    }

    #[test]
    fn flag_anomalies_no_outliers_when_all_same() {
        let vectors: Vec<BehaviorVector> = (0..10)
            .map(|_| BehaviorVector {
                agent_id: AgentId::new(),
                window_start: 0,
                window_end: 50,
                action_counts: {
                    let mut m = BTreeMap::new();
                    m.insert("Gather".to_owned(), 10);
                    m
                },
                interaction_counts: BTreeMap::new(),
                resource_deltas: BTreeMap::new(),
            })
            .collect();

        let clusters = cluster_behavior_vectors(&vectors, 1, 50);
        let flags = flag_anomalies(&vectors, &clusters, 50);
        // All vectors are identical -- no outliers.
        assert!(flags.is_empty());
    }

    #[test]
    fn flag_anomalies_detects_outlier() {
        let mut vectors: Vec<BehaviorVector> = (0..10)
            .map(|_| BehaviorVector {
                agent_id: AgentId::new(),
                window_start: 0,
                window_end: 50,
                action_counts: {
                    let mut m = BTreeMap::new();
                    m.insert("Gather".to_owned(), 10);
                    m
                },
                interaction_counts: BTreeMap::new(),
                resource_deltas: BTreeMap::new(),
            })
            .collect();

        // Add one extreme outlier.
        vectors.push(BehaviorVector {
            agent_id: AgentId::new(),
            window_start: 0,
            window_end: 50,
            action_counts: {
                let mut m = BTreeMap::new();
                m.insert("Gather".to_owned(), 10000);
                m
            },
            interaction_counts: BTreeMap::new(),
            resource_deltas: BTreeMap::new(),
        });

        let clusters = cluster_behavior_vectors(&vectors, 1, 50);
        let flags = flag_anomalies(&vectors, &clusters, 50);
        // The outlier should be flagged.
        assert!(!flags.is_empty());
    }
}
