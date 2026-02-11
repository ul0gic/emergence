//! Social construct detection API handlers.
//!
//! These endpoints analyze the current simulation state to detect emergent
//! social patterns. All data is computed on-the-fly from the in-memory
//! [`SimulationSnapshot`] -- no additional storage is required.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET` | `/api/social/beliefs` | Detected belief systems |
//! | `GET` | `/api/social/governance` | Governance structures |
//! | `GET` | `/api/social/families` | Family units and lineage |
//! | `GET` | `/api/social/economy` | Economic classification |
//! | `GET` | `/api/social/crime` | Crime and justice stats |
//!
//! [`SimulationSnapshot`]: crate::state::SimulationSnapshot

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use rust_decimal::Decimal;

use crate::error::ObserverError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/social/beliefs -- detected belief systems
// ---------------------------------------------------------------------------

/// Detect belief systems from agent knowledge and memory patterns.
///
/// A "belief system" is inferred when multiple agents share a cluster of
/// related knowledge concepts and have memories referencing spiritual,
/// religious, or philosophical themes. This is a heuristic detection --
/// the agents themselves do not explicitly declare beliefs.
pub async fn beliefs(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    // Detect belief systems by looking for shared knowledge clusters.
    let belief_keywords = [
        "religion", "spiritual", "ritual", "prayer", "deity",
        "afterlife", "sacred", "divine", "worship", "ceremony",
        "belief", "faith", "philosophy", "moral",
    ];

    // Count how many agents hold each knowledge concept.
    let mut knowledge_holders: BTreeMap<String, Vec<emergence_types::AgentId>> = BTreeMap::new();
    for agent_state in snapshot.agent_states.values() {
        for concept in &agent_state.knowledge {
            knowledge_holders
                .entry(concept.clone())
                .or_default()
                .push(agent_state.agent_id);
        }
    }

    // Group belief-related concepts into belief systems.
    let mut belief_systems: Vec<serde_json::Value> = Vec::new();
    let mut belief_events: Vec<serde_json::Value> = Vec::new();
    let mut system_idx: u32 = 0;

    // Check for clusters of belief-related knowledge shared by 2+ agents.
    let mut belief_concepts: Vec<(String, Vec<emergence_types::AgentId>)> = knowledge_holders
        .into_iter()
        .filter(|(concept, holders)| {
            holders.len() >= 2
                && belief_keywords
                    .iter()
                    .any(|kw| concept.to_lowercase().contains(kw))
        })
        .collect();

    // Sort by number of holders descending.
    belief_concepts.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    // Each qualifying concept cluster becomes a belief system.
    let mut seen_adherents: BTreeSet<emergence_types::AgentId> = BTreeSet::new();
    for (concept, holders) in &belief_concepts {
        system_idx = system_idx.saturating_add(1);
        let unique_holders: Vec<&emergence_types::AgentId> = holders
            .iter()
            .filter(|id| !seen_adherents.contains(id))
            .collect();

        if unique_holders.is_empty() {
            continue;
        }

        for &&id in &unique_holders {
            seen_adherents.insert(id);
        }

        #[allow(clippy::cast_possible_truncation)]
        let adherent_count = unique_holders.len() as u32;

        // Determine founding tick from earliest agent who holds this knowledge.
        let founded_tick = unique_holders
            .iter()
            .filter_map(|id| snapshot.agents.get(id).map(|a| a.born_at_tick))
            .min()
            .unwrap_or(0);

        let system_id = format!("belief-{system_idx}");
        belief_systems.push(serde_json::json!({
            "id": system_id,
            "name": format_belief_name(concept),
            "themes": [concept],
            "adherent_count": adherent_count,
            "founded_at_tick": founded_tick,
        }));

        belief_events.push(serde_json::json!({
            "tick": founded_tick,
            "event_type": "founded",
            "belief_system_id": system_id,
            "belief_system_name": format_belief_name(concept),
            "description": format!("A belief system around '{}' emerged among {} agents", concept, adherent_count),
            "agent_id": unique_holders.first().map(std::string::ToString::to_string),
        }));
    }

    Ok(Json(serde_json::json!({
        "belief_systems": belief_systems,
        "belief_events": belief_events,
    })))
}

/// Format a knowledge concept into a more readable belief system name.
fn format_belief_name(concept: &str) -> String {
    let mut chars = concept.replace('_', " ").chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        *first = first.to_uppercase().next().unwrap_or(*first);
    }
    let base: String = chars.into_iter().collect();
    format!("The Way of {base}")
}

// ---------------------------------------------------------------------------
// GET /api/social/governance -- governance structures
// ---------------------------------------------------------------------------

/// Detect governance patterns from agent relationships, knowledge,
/// and group dynamics.
///
/// Governance type is inferred from:
/// - Number of agents with leadership-related knowledge
/// - Relationship density and hierarchy
/// - Population size and knowledge distribution
#[allow(clippy::too_many_lines)]
pub async fn governance(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let governance_knowledge = [
        "governance", "leadership", "law", "legislation", "territorial_claim",
        "group_formation", "written_language", "diplomacy",
    ];

    // Find agents with governance-related knowledge.
    let mut leaders: Vec<serde_json::Value> = Vec::new();
    let mut rules: Vec<String> = Vec::new();
    let mut governance_agents: Vec<(emergence_types::AgentId, u32)> = Vec::new();

    for agent_state in snapshot.agent_states.values() {
        let gov_knowledge_count = agent_state
            .knowledge
            .iter()
            .filter(|k| governance_knowledge.iter().any(|gk| k.contains(gk)))
            .count();
        if gov_knowledge_count > 0 {
            #[allow(clippy::cast_possible_truncation)]
            governance_agents.push((agent_state.agent_id, gov_knowledge_count as u32));
        }
    }

    // Sort by governance knowledge count descending.
    governance_agents.sort_by(|a, b| b.1.cmp(&a.1));

    // Classify governance type based on population dynamics.
    let alive_count = snapshot
        .agent_states
        .values()
        .filter(|s| snapshot.agents.get(&s.agent_id).is_some_and(|a| a.died_at_tick.is_none()))
        .count();

    let gov_type = if governance_agents.is_empty() {
        "Anarchy"
    } else if governance_agents.len() == 1 || alive_count < 5 {
        "Chieftainship"
    } else if governance_agents.len() <= 3 {
        "Council"
    } else {
        "Oligarchy"
    };

    // Build leader list (top governance-knowledgeable agents).
    for (agent_id, _score) in governance_agents.iter().take(5) {
        if let Some(agent) = snapshot.agents.get(agent_id) {
            let role = if leaders.is_empty() {
                match gov_type {
                    "Chieftainship" => "Chief",
                    "Council" => "Council Elder",
                    "Oligarchy" => "Oligarch",
                    _ => "Leader",
                }
            } else {
                "Advisor"
            };

            leaders.push(serde_json::json!({
                "agent_id": agent_id.to_string(),
                "agent_name": agent.name,
                "role": role,
                "since_tick": agent.born_at_tick,
            }));
        }
    }

    // Detect rules from widespread knowledge.
    let knowledge_counts: BTreeMap<String, usize> = {
        let mut counts = BTreeMap::new();
        for agent_state in snapshot.agent_states.values() {
            for concept in &agent_state.knowledge {
                let entry = counts.entry(concept.clone()).or_insert(0usize);
                *entry = entry.saturating_add(1);
            }
        }
        counts
    };

    let threshold = alive_count / 2;
    for (concept, count) in &knowledge_counts {
        if *count > threshold
            && governance_knowledge.iter().any(|gk| concept.contains(gk))
        {
            rules.push(format!("Shared concept: {concept} (known by {count} agents)"));
        }
    }

    // Compute stability score (0.0 to 1.0) based on ratio of
    // governance-knowledgeable agents to total population.
    #[allow(clippy::arithmetic_side_effects)]
    let stability = if alive_count == 0 {
        Decimal::ZERO
    } else {
        let gov_count = governance_agents.len().min(alive_count);
        #[allow(clippy::cast_possible_truncation)]
        let ratio = Decimal::from(gov_count as u32) / Decimal::from(alive_count as u32);
        ratio.min(Decimal::ONE)
    };

    Ok(Json(serde_json::json!({
        "governance_type": gov_type,
        "leaders": leaders,
        "rules": rules,
        "stability_score": stability,
        "recent_events": [],
    })))
}

// ---------------------------------------------------------------------------
// GET /api/social/families -- family units and lineage
// ---------------------------------------------------------------------------

/// Detect family units and build lineage trees from agent parent data.
///
/// Family units are groups of agents connected by parent-child
/// relationships. Lineage is the full tree of parent-child connections.
#[allow(clippy::too_many_lines)]
pub async fn families(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let (lineage, max_generation) = build_lineage(&snapshot);
    let (family_units, parent_pair_count) = build_family_units(&snapshot);

    // Count orphans (agents with no parents who are not seed agents).
    let orphan_count: u32 = snapshot
        .agents
        .values()
        .filter(|a| {
            a.parent_a.is_none()
                && a.parent_b.is_none()
                && a.generation > 0
                && a.died_at_tick.is_none()
        })
        .count()
        .try_into()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    let unit_count = family_units.len() as u32;

    let avg_size = if family_units.is_empty() {
        Decimal::ZERO
    } else {
        let total_members: u32 = family_units
            .iter()
            .filter_map(|f| f.get("members").and_then(|m| m.as_array()).map(Vec::len))
            .sum::<usize>()
            .try_into()
            .unwrap_or(0);
        #[allow(clippy::arithmetic_side_effects)]
        let avg = Decimal::from(total_members) / Decimal::from(unit_count);
        avg
    };

    Ok(Json(serde_json::json!({
        "unit_count": unit_count,
        "avg_size": avg_size,
        "marriage_count": parent_pair_count,
        "divorce_count": 0,
        "orphan_count": orphan_count,
        "longest_lineage": max_generation,
        "families": family_units,
        "lineage": lineage,
    })))
}

/// Build lineage nodes from agent parent data.
fn build_lineage(
    snapshot: &crate::state::SimulationSnapshot,
) -> (Vec<serde_json::Value>, u32) {
    // Build parent->children map.
    let mut children_of: BTreeMap<emergence_types::AgentId, Vec<emergence_types::AgentId>> =
        BTreeMap::new();

    for agent in snapshot.agents.values() {
        if let Some(parent_a) = agent.parent_a {
            children_of.entry(parent_a).or_default().push(agent.id);
        }
        if let Some(parent_b) = agent.parent_b {
            children_of.entry(parent_b).or_default().push(agent.id);
        }
    }

    let mut lineage: Vec<serde_json::Value> = Vec::new();
    let mut max_generation: u32 = 0;

    for agent in snapshot.agents.values() {
        let children: Vec<String> = children_of
            .get(&agent.id)
            .map(|kids| kids.iter().map(ToString::to_string).collect())
            .unwrap_or_default();

        lineage.push(serde_json::json!({
            "agent_id": agent.id.to_string(),
            "agent_name": agent.name,
            "parent_a": agent.parent_a.map(|id| id.to_string()),
            "parent_b": agent.parent_b.map(|id| id.to_string()),
            "generation": agent.generation,
            "alive": agent.died_at_tick.is_none(),
            "children": children,
        }));

        if agent.generation > max_generation {
            max_generation = agent.generation;
        }
    }

    (lineage, max_generation)
}

/// Build family units from parent pairs.
fn build_family_units(
    snapshot: &crate::state::SimulationSnapshot,
) -> (Vec<serde_json::Value>, u32) {
    let mut family_units: Vec<serde_json::Value> = Vec::new();
    let mut family_idx: u32 = 0;

    // For each pair of parents, create a family unit.
    let mut parent_pairs: BTreeMap<
        (emergence_types::AgentId, emergence_types::AgentId),
        Vec<emergence_types::AgentId>,
    > = BTreeMap::new();

    for agent in snapshot.agents.values() {
        if let (Some(pa), Some(pb)) = (agent.parent_a, agent.parent_b) {
            let key = if pa < pb { (pa, pb) } else { (pb, pa) };
            parent_pairs.entry(key).or_default().push(agent.id);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let pair_count = parent_pairs.len() as u32;

    for ((pa, pb), children) in &parent_pairs {
        family_idx = family_idx.saturating_add(1);

        let mut members: Vec<String> = vec![pa.to_string(), pb.to_string()];
        for child in children {
            members.push(child.to_string());
        }

        let head_name = snapshot
            .agents
            .get(pa)
            .map_or_else(|| String::from("Unknown"), |a| a.name.clone());

        let formed_tick = children
            .iter()
            .filter_map(|id| snapshot.agents.get(id).map(|a| a.born_at_tick))
            .min()
            .unwrap_or(0);

        family_units.push(serde_json::json!({
            "id": format!("family-{family_idx}"),
            "name": format!("{head_name} Family"),
            "members": members,
            "head": pa.to_string(),
            "formed_at_tick": formed_tick,
        }));
    }

    (family_units, pair_count)
}

// ---------------------------------------------------------------------------
// GET /api/social/economy -- economic classification
// ---------------------------------------------------------------------------

/// Classify the economic model from trade patterns, resource distribution,
/// and currency adoption.
///
/// Looks at agent inventories, trade-related knowledge, and resource
/// distribution across locations to determine the economic model type.
#[allow(clippy::too_many_lines)]
pub async fn economy(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let alive_count = snapshot
        .agent_states
        .values()
        .filter(|s| snapshot.agents.get(&s.agent_id).is_some_and(|a| a.died_at_tick.is_none()))
        .count();

    // Check for currency-related knowledge and resources.
    let currency_holders = snapshot
        .agent_states
        .values()
        .filter(|s| s.inventory.contains_key(&emergence_types::Resource::CurrencyToken))
        .count();

    let trade_knowledge_holders = snapshot
        .agent_states
        .values()
        .filter(|s| {
            s.knowledge.iter().any(|k| {
                k.contains("trade") || k.contains("barter") || k.contains("currency")
                    || k.contains("market") || k.contains("commerce")
            })
        })
        .count();

    // Classify economic model.
    #[allow(clippy::arithmetic_side_effects)]
    let currency_adoption_pct = if alive_count == 0 {
        Decimal::ZERO
    } else {
        #[allow(clippy::cast_possible_truncation)]
        let pct = Decimal::from(currency_holders as u32) / Decimal::from(alive_count as u32);
        pct
    };

    // 0.50 threshold for currency economy classification.
    let half = Decimal::new(50, 2);
    let model_type = if currency_holders > 0 && currency_adoption_pct > half {
        "Currency"
    } else if trade_knowledge_holders > alive_count / 2 {
        "Barter"
    } else if trade_knowledge_holders > 0 {
        "Gift"
    } else {
        "Subsistence"
    };

    // Detect currency resource (if any).
    let currency_resource: Option<&str> = if currency_holders > 0 {
        Some("CurrencyToken")
    } else {
        None
    };

    // Compute trade volume (sum of trade-related action events).
    let trade_events = snapshot
        .events
        .iter()
        .filter(|e| {
            e.details.as_object().is_some_and(|details| {
                details
                    .get("action_type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|t| t.contains("Trade") || t.contains("Give"))
            })
        })
        .count();

    // Build market locations (locations with the most agents and resources).
    let mut location_trade_counts: BTreeMap<emergence_types::LocationId, u32> = BTreeMap::new();
    for agent_state in snapshot.agent_states.values() {
        let entry = location_trade_counts
            .entry(agent_state.location_id)
            .or_insert(0u32);
        *entry = entry.saturating_add(1);
    }

    let mut market_locations: Vec<serde_json::Value> = Vec::new();
    let mut sorted_locations: Vec<_> = location_trade_counts.into_iter().collect();
    sorted_locations.sort_by(|a, b| b.1.cmp(&a.1));

    for (loc_id, agent_count) in sorted_locations.into_iter().take(5) {
        if let Some(location) = snapshot.locations.get(&loc_id) {
            let primary = location
                .base_resources
                .values()
                .max_by_key(|n| n.available)
                .map_or_else(
                    || String::from("Water"),
                    |n| format!("{:?}", n.resource),
                );

            market_locations.push(serde_json::json!({
                "location_id": loc_id.to_string(),
                "location_name": location.name,
                "trade_volume": agent_count,
                "primary_resource": primary,
            }));
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let trade_volume = trade_events as u32;

    Ok(Json(serde_json::json!({
        "model_type": model_type,
        "currency_resource": currency_resource,
        "currency_adoption_pct": currency_adoption_pct,
        "trade_volume": trade_volume,
        "trade_volume_history": [],
        "market_locations": market_locations,
    })))
}

// ---------------------------------------------------------------------------
// GET /api/social/crime -- crime and justice statistics
// ---------------------------------------------------------------------------

/// Detect crime patterns from theft and combat action events.
///
/// Crime is detected by looking at action events of type `Steal`, `Attack`,
/// and `Rob`. Justice type is inferred from governance knowledge and
/// whether punishment events exist.
#[allow(clippy::too_many_lines)]
pub async fn crime(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let alive_count = snapshot
        .agent_states
        .values()
        .filter(|s| snapshot.agents.get(&s.agent_id).is_some_and(|a| a.died_at_tick.is_none()))
        .count();

    let crime_types = ["Steal", "Attack", "Rob", "Theft", "Combat"];
    let (crime_counts, crime_by_agent, crime_by_location, total_crimes) =
        count_crimes(&snapshot.events, &crime_types);

    // Compute crime rate.
    let crime_rate = if alive_count == 0 {
        Decimal::ZERO
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::arithmetic_side_effects)]
        let rate = Decimal::from(total_crimes) / Decimal::from(alive_count as u32);
        rate
    };

    // Determine justice type from governance knowledge.
    let has_governance = snapshot
        .agent_states
        .values()
        .any(|s| s.knowledge.iter().any(|k| k.contains("governance") || k.contains("law")));

    let has_written_law = snapshot
        .agent_states
        .values()
        .any(|s| s.knowledge.iter().any(|k| k.contains("legislation") || k.contains("codified")));

    let justice_type = if has_written_law {
        "Codified"
    } else if has_governance {
        "Elder"
    } else if total_crimes > 0 {
        "Vigilante"
    } else {
        "None"
    };

    // Build common crimes list.
    let mut common_crimes: Vec<serde_json::Value> = crime_counts
        .iter()
        .map(|(crime_type, count)| {
            serde_json::json!({
                "crime_type": crime_type,
                "count": count,
            })
        })
        .collect();
    common_crimes.sort_by(|a, b| {
        let count_a = a.get("count").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let count_b = b.get("count").and_then(serde_json::Value::as_u64).unwrap_or(0);
        count_b.cmp(&count_a)
    });

    // Build serial offenders list (agents with 2+ crimes).
    let serial_offenders = build_serial_offenders(
        &crime_by_agent,
        &snapshot.agents,
        &snapshot.events,
        &crime_types,
    );

    // Build crime hotspots.
    let hotspots = build_crime_hotspots(&crime_by_location, &snapshot.locations);

    Ok(Json(serde_json::json!({
        "crime_rate": crime_rate,
        "crime_rate_history": [],
        "detection_rate": 0.0,
        "punishment_rate": 0.0,
        "justice_type": justice_type,
        "common_crimes": common_crimes,
        "serial_offenders": serial_offenders,
        "hotspots": hotspots,
    })))
}

/// Count crimes by type, agent, and location from the event log.
#[allow(clippy::type_complexity)]
fn count_crimes(
    events: &[emergence_types::Event],
    crime_types: &[&str],
) -> (
    BTreeMap<String, u32>,
    BTreeMap<emergence_types::AgentId, u32>,
    BTreeMap<emergence_types::LocationId, u32>,
    u32,
) {
    let mut crime_counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut crime_by_agent: BTreeMap<emergence_types::AgentId, u32> = BTreeMap::new();
    let mut crime_by_location: BTreeMap<emergence_types::LocationId, u32> = BTreeMap::new();
    let mut total_crimes: u32 = 0;

    for event in events {
        let is_crime = event.details.as_object().is_some_and(|details| {
            details
                .get("action_type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|t| crime_types.iter().any(|ct| t.contains(ct)))
        });

        if is_crime {
            let action_type = event
                .details
                .as_object()
                .and_then(|d| d.get("action_type"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Unknown")
                .to_owned();

            let entry = crime_counts.entry(action_type).or_insert(0u32);
            *entry = entry.saturating_add(1);
            total_crimes = total_crimes.saturating_add(1);

            if let Some(agent_id) = event.agent_id {
                let agent_entry = crime_by_agent.entry(agent_id).or_insert(0u32);
                *agent_entry = agent_entry.saturating_add(1);
            }

            if let Some(loc_id) = event.location_id {
                let loc_entry = crime_by_location.entry(loc_id).or_insert(0u32);
                *loc_entry = loc_entry.saturating_add(1);
            }
        }
    }

    (crime_counts, crime_by_agent, crime_by_location, total_crimes)
}

/// Build the serial offenders list from crime-by-agent counts.
fn build_serial_offenders(
    crime_by_agent: &BTreeMap<emergence_types::AgentId, u32>,
    agents: &BTreeMap<emergence_types::AgentId, emergence_types::Agent>,
    events: &[emergence_types::Event],
    crime_types: &[&str],
) -> Vec<serde_json::Value> {
    let mut serial_offenders: Vec<serde_json::Value> = Vec::new();
    let mut sorted_offenders: Vec<_> = crime_by_agent
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();
    sorted_offenders.sort_by(|a, b| b.1.cmp(a.1));

    for (agent_id, count) in sorted_offenders.into_iter().take(10) {
        let agent_name = agents
            .get(agent_id)
            .map_or_else(|| String::from("Unknown"), |a| a.name.clone());

        let last_tick = events
            .iter()
            .rev()
            .find(|e| {
                e.agent_id == Some(*agent_id)
                    && e.details
                        .as_object()
                        .and_then(|d| d.get("action_type"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|t| crime_types.iter().any(|ct| t.contains(ct)))
            })
            .map_or(0, |e| e.tick);

        serial_offenders.push(serde_json::json!({
            "agent_id": agent_id.to_string(),
            "agent_name": agent_name,
            "offense_count": count,
            "last_offense_tick": last_tick,
        }));
    }

    serial_offenders
}

/// Build crime hotspots from location crime counts.
fn build_crime_hotspots(
    crime_by_location: &BTreeMap<emergence_types::LocationId, u32>,
    locations: &BTreeMap<emergence_types::LocationId, emergence_types::Location>,
) -> Vec<serde_json::Value> {
    let mut hotspots: Vec<serde_json::Value> = Vec::new();
    let mut sorted_hotspots: Vec<_> = crime_by_location.iter().collect();
    sorted_hotspots.sort_by(|a, b| b.1.cmp(a.1));

    for (loc_id, count) in sorted_hotspots.into_iter().take(5) {
        let loc_name = locations
            .get(loc_id)
            .map_or_else(|| String::from("Unknown"), |l| l.name.clone());

        hotspots.push(serde_json::json!({
            "location_id": loc_id.to_string(),
            "location_name": loc_name,
            "crime_count": count,
        }));
    }

    hotspots
}
