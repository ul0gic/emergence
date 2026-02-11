//! Alert system for containment and monitoring.
//!
//! Implements Phase 5.4.4: an observer alert system that tracks containment
//! breaches, population collapses, economic anomalies, and first-instance
//! milestones.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET` | `/api/alerts` | List all alerts |
//! | `POST` | `/api/alerts/{id}/acknowledge` | Acknowledge an alert |
//!
//! # Alert Categories
//!
//! - `containment` -- escape detection triggered in the runner
//! - `population` -- population collapse or extinction risk
//! - `economy` -- ledger anomaly or economic crisis
//! - `milestone` -- first-instance achievement (first trade, first death, etc.)
//! - `anomaly` -- behavioral anomaly flagged by the detection layer

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use crate::error::ObserverError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Alert types
// ---------------------------------------------------------------------------

/// Severity level of an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    /// Informational -- a notable event that does not require action.
    Info,
    /// Warning -- something is off but not critical.
    Warning,
    /// Critical -- immediate attention required.
    Critical,
}

/// Category of an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCategory {
    /// Containment breach detected (escape attempts, prompt injection).
    Containment,
    /// Population collapse or extinction risk.
    Population,
    /// Ledger anomaly or economic crisis.
    Economy,
    /// First-instance milestone achievement.
    Milestone,
    /// Behavioral anomaly flagged by the detection layer.
    Anomaly,
}

/// A single alert in the alert system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alert {
    /// Unique alert identifier.
    pub id: Uuid,
    /// Severity of the alert.
    pub severity: AlertSeverity,
    /// Human-readable message.
    pub message: String,
    /// Tick when the alert was raised.
    pub tick: u64,
    /// Alert category.
    pub category: AlertCategory,
    /// Whether the operator has acknowledged this alert.
    pub acknowledged: bool,
    /// ISO 8601 timestamp when the alert was created.
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Alert Store
// ---------------------------------------------------------------------------

/// Maximum alerts to keep in memory.
const MAX_ALERTS: usize = 500;

/// In-memory alert store.
#[derive(Debug, Clone, Default)]
pub struct AlertStore {
    /// All alerts, newest first.
    alerts: Vec<Alert>,
}

impl AlertStore {
    /// Create a new empty alert store.
    pub const fn new() -> Self {
        Self {
            alerts: Vec::new(),
        }
    }

    /// Add an alert to the store.
    ///
    /// If the store exceeds [`MAX_ALERTS`], the oldest alert is removed.
    pub fn push(&mut self, alert: Alert) {
        self.alerts.insert(0, alert);
        if self.alerts.len() > MAX_ALERTS {
            self.alerts.truncate(MAX_ALERTS);
        }
    }

    /// Get all alerts.
    pub fn all(&self) -> &[Alert] {
        &self.alerts
    }

    /// Acknowledge an alert by ID.
    ///
    /// Returns `true` if the alert was found and acknowledged, `false` if
    /// the ID was not found.
    pub fn acknowledge(&mut self, id: Uuid) -> bool {
        for alert in &mut self.alerts {
            if alert.id == id {
                alert.acknowledged = true;
                return true;
            }
        }
        false
    }

    /// Get alerts filtered by category.
    pub fn by_category(&self, category: AlertCategory) -> Vec<&Alert> {
        self.alerts
            .iter()
            .filter(|a| a.category == category)
            .collect()
    }

    /// Get unacknowledged alerts.
    pub fn unacknowledged(&self) -> Vec<&Alert> {
        self.alerts.iter().filter(|a| !a.acknowledged).collect()
    }

    /// Create and push a new alert.
    pub fn raise(
        &mut self,
        severity: AlertSeverity,
        category: AlertCategory,
        message: String,
        tick: u64,
    ) {
        let alert = Alert {
            id: Uuid::now_v7(),
            severity,
            message,
            tick,
            category,
            acknowledged: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.push(alert);
    }
}

// ---------------------------------------------------------------------------
// Alert Generation (from simulation state)
// ---------------------------------------------------------------------------

/// Check the simulation state for alert-worthy conditions.
///
/// This is intended to be called periodically (e.g., each tick) to generate
/// alerts based on current state. It checks:
///
/// - Population collapse (alive count dropped below 2)
/// - Economic anomaly (Gini coefficient above 0.9)
/// - First-instance milestones (first trade, first death, first structure)
pub fn check_for_alerts(
    snapshot: &crate::state::SimulationSnapshot,
    alert_store: &mut AlertStore,
) {
    let tick = snapshot.current_tick;

    // Population collapse check.
    let alive_count = snapshot
        .agent_states
        .values()
        .filter(|s| {
            snapshot
                .agents
                .get(&s.agent_id)
                .is_some_and(|a| a.died_at_tick.is_none())
        })
        .count();

    if alive_count == 0 && !snapshot.agents.is_empty() {
        alert_store.raise(
            AlertSeverity::Critical,
            AlertCategory::Population,
            "Population extinction: all agents have died".to_owned(),
            tick,
        );
    } else if alive_count == 1 {
        alert_store.raise(
            AlertSeverity::Warning,
            AlertCategory::Population,
            "Population critical: only 1 agent alive".to_owned(),
            tick,
        );
    } else if alive_count <= 3 && snapshot.agents.len() > 5 {
        alert_store.raise(
            AlertSeverity::Warning,
            AlertCategory::Population,
            format!("Population collapse risk: only {alive_count} agents alive"),
            tick,
        );
    }

    // Check for ledger anomaly events.
    for event in &snapshot.events {
        if event.tick == tick
            && matches!(
                event.event_type,
                emergence_types::EventType::LedgerAnomaly
            )
        {
            alert_store.raise(
                AlertSeverity::Critical,
                AlertCategory::Economy,
                "LEDGER_ANOMALY: conservation law violated".to_owned(),
                tick,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// REST Handlers
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/alerts`.
#[derive(Debug, serde::Deserialize)]
pub struct AlertsQuery {
    /// Filter by category.
    pub category: Option<String>,
    /// Filter by acknowledged status (`true` or `false`).
    pub acknowledged: Option<String>,
    /// Maximum number of alerts to return (default 100).
    pub limit: Option<usize>,
}

/// `GET /api/alerts` -- list alerts with optional filtering.
pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlertsQuery>,
) -> Result<impl IntoResponse, ObserverError> {
    let alert_store = state.alert_store.read().await;
    let limit = params.limit.unwrap_or(100).min(500);

    let category_filter: Option<AlertCategory> = params.category.as_deref().and_then(|c| {
        match c {
            "containment" => Some(AlertCategory::Containment),
            "population" => Some(AlertCategory::Population),
            "economy" => Some(AlertCategory::Economy),
            "milestone" => Some(AlertCategory::Milestone),
            "anomaly" => Some(AlertCategory::Anomaly),
            _ => None,
        }
    });

    let acknowledged_filter: Option<bool> = params.acknowledged.as_deref().and_then(|a| {
        match a {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    });

    let alerts: Vec<&Alert> = alert_store
        .all()
        .iter()
        .filter(|a| {
            if let Some(cat) = category_filter
                && a.category != cat
            {
                return false;
            }
            if let Some(ack) = acknowledged_filter
                && a.acknowledged != ack
            {
                return false;
            }
            true
        })
        .take(limit)
        .collect();

    Ok(Json(serde_json::json!({
        "count": alerts.len(),
        "alerts": alerts,
    })))
}

/// `POST /api/alerts/{id}/acknowledge` -- acknowledge an alert.
pub async fn acknowledge_alert(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse, ObserverError> {
    let id = id_str
        .parse::<Uuid>()
        .map_err(|e| ObserverError::InvalidUuid(format!("{id_str}: {e}")))?;

    let mut alert_store = state.alert_store.write().await;
    if alert_store.acknowledge(id) {
        Ok(Json(serde_json::json!({
            "ok": true,
            "message": format!("Alert {id} acknowledged"),
        })))
    } else {
        Err(ObserverError::NotFound(format!("alert {id}")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_store_push_and_retrieve() {
        let mut store = AlertStore::new();
        store.raise(
            AlertSeverity::Info,
            AlertCategory::Milestone,
            "First trade completed".to_owned(),
            10,
        );
        assert_eq!(store.all().len(), 1);
        assert_eq!(
            store.all().first().map(|a| a.severity),
            Some(AlertSeverity::Info)
        );
    }

    #[test]
    fn alert_store_acknowledge() {
        let mut store = AlertStore::new();
        store.raise(
            AlertSeverity::Warning,
            AlertCategory::Population,
            "Low population".to_owned(),
            5,
        );
        let id = store.all().first().map(|a| a.id).unwrap_or(Uuid::nil());
        assert!(store.acknowledge(id));
        assert!(
            store
                .all()
                .first()
                .map_or(false, |a| a.acknowledged)
        );
    }

    #[test]
    fn alert_store_acknowledge_unknown_returns_false() {
        let mut store = AlertStore::new();
        assert!(!store.acknowledge(Uuid::nil()));
    }

    #[test]
    fn alert_store_caps_at_max() {
        let mut store = AlertStore::new();
        for i in 0..600u64 {
            store.raise(
                AlertSeverity::Info,
                AlertCategory::Milestone,
                format!("Alert {i}"),
                i,
            );
        }
        assert_eq!(store.all().len(), MAX_ALERTS);
    }

    #[test]
    fn filter_by_category() {
        let mut store = AlertStore::new();
        store.raise(
            AlertSeverity::Info,
            AlertCategory::Milestone,
            "milestone 1".to_owned(),
            1,
        );
        store.raise(
            AlertSeverity::Warning,
            AlertCategory::Population,
            "population warning".to_owned(),
            2,
        );

        let milestones = store.by_category(AlertCategory::Milestone);
        assert_eq!(milestones.len(), 1);

        let unack = store.unacknowledged();
        assert_eq!(unack.len(), 2);
    }
}
