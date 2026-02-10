//! Operator control state for runtime simulation management.
//!
//! This module provides shared atomic state used by the tick loop and the
//! operator REST API. The operator can pause/resume, change tick speed,
//! inject events, and trigger a clean shutdown -- all without stopping
//! the process.
//!
//! # Architecture
//!
//! All mutable control fields use [`std::sync::atomic`] types wrapped in
//! [`Arc`] so they can be shared between the tick loop task and the Axum
//! handler tasks without locks on the hot path.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

use crate::config::SimulationBoundsConfig;

/// Reason why the simulation ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimulationEndReason {
    /// Reached the configured `max_ticks` limit.
    MaxTicksReached,
    /// Reached the configured `max_real_time_seconds` limit.
    MaxRealTimeReached,
    /// An operator issued a stop command.
    OperatorStop,
    /// All agents are dead.
    Extinction,
}

/// An operator-injected event that will be applied at the start of the
/// next tick's World Wake phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedEvent {
    /// The type of event to inject (e.g. "plague", "resource\_boom").
    pub event_type: String,
    /// Optional target region for the event.
    pub target_region: Option<String>,
    /// Optional severity or magnitude (interpretation depends on event type).
    pub severity: Option<String>,
    /// Free-form description for the event log.
    pub description: Option<String>,
}

/// Shared operator control state.
///
/// This struct is wrapped in [`Arc`] and shared between the tick loop
/// and operator API handlers. Atomic fields are used for lock-free
/// reads on the tick loop hot path.
#[derive(Debug)]
pub struct OperatorState {
    /// Whether the simulation is currently paused.
    paused: AtomicBool,

    /// Notification used to wake the tick loop when resumed.
    resume_notify: Notify,

    /// Whether a stop has been requested.
    stop_requested: AtomicBool,

    /// Current tick interval in milliseconds (runtime-adjustable).
    tick_interval_ms: AtomicU64,

    /// Wall-clock time when the simulation started.
    started_at: DateTime<Utc>,

    /// Maximum number of ticks (0 = unlimited).
    max_ticks: u64,

    /// Maximum wall-clock seconds (0 = unlimited).
    max_real_time_seconds: u64,

    /// Queue of operator-injected events awaiting processing.
    injected_events: Mutex<Vec<InjectedEvent>>,

    /// Reason the simulation ended, if it has.
    end_reason: Mutex<Option<SimulationEndReason>>,
}

impl OperatorState {
    /// Create a new operator state from configuration.
    pub fn new(tick_interval_ms: u64, bounds: &SimulationBoundsConfig) -> Self {
        Self {
            paused: AtomicBool::new(false),
            resume_notify: Notify::new(),
            stop_requested: AtomicBool::new(false),
            tick_interval_ms: AtomicU64::new(tick_interval_ms),
            started_at: Utc::now(),
            max_ticks: bounds.max_ticks,
            max_real_time_seconds: bounds.max_real_time_seconds,
            injected_events: Mutex::new(Vec::new()),
            end_reason: Mutex::new(None),
        }
    }

    // -----------------------------------------------------------------------
    // Pause / Resume
    // -----------------------------------------------------------------------

    /// Check whether the simulation is paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Pause the simulation. The tick loop will sleep until resumed.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    /// Resume the simulation and wake the tick loop.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.resume_notify.notify_one();
    }

    /// Wait until the simulation is no longer paused.
    ///
    /// Returns immediately if not paused. Otherwise blocks until
    /// [`resume`](Self::resume) is called.
    pub async fn wait_if_paused(&self) {
        while self.paused.load(Ordering::Acquire) {
            self.resume_notify.notified().await;
        }
    }

    // -----------------------------------------------------------------------
    // Stop
    // -----------------------------------------------------------------------

    /// Request a clean simulation stop.
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
    }

    /// Check whether a stop has been requested.
    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Acquire)
    }

    /// Record the reason the simulation ended.
    pub async fn set_end_reason(&self, reason: SimulationEndReason) {
        let mut guard = self.end_reason.lock().await;
        *guard = Some(reason);
    }

    /// Get the reason the simulation ended, if it has.
    pub async fn end_reason(&self) -> Option<SimulationEndReason> {
        self.end_reason.lock().await.clone()
    }

    // -----------------------------------------------------------------------
    // Tick Speed
    // -----------------------------------------------------------------------

    /// Get the current tick interval in milliseconds.
    pub fn tick_interval_ms(&self) -> u64 {
        self.tick_interval_ms.load(Ordering::Acquire)
    }

    /// Set the tick interval in milliseconds. Must be at least 100ms.
    ///
    /// Returns the previous interval on success, or `None` if the
    /// value was rejected (below 100ms).
    pub fn set_tick_interval_ms(&self, ms: u64) -> Option<u64> {
        if ms < 100 {
            return None;
        }
        let prev = self.tick_interval_ms.swap(ms, Ordering::AcqRel);
        Some(prev)
    }

    // -----------------------------------------------------------------------
    // Boundaries
    // -----------------------------------------------------------------------

    /// Check whether the tick limit has been reached.
    ///
    /// Returns `true` if `max_ticks > 0` and `current_tick >= max_ticks`.
    pub const fn tick_limit_reached(&self, current_tick: u64) -> bool {
        self.max_ticks > 0 && current_tick >= self.max_ticks
    }

    /// Check whether the wall-clock time limit has been reached.
    ///
    /// Returns `true` if `max_real_time_seconds > 0` and the elapsed
    /// seconds since start exceed the limit.
    pub fn time_limit_reached(&self) -> bool {
        if self.max_real_time_seconds == 0 {
            return false;
        }
        let elapsed = Utc::now()
            .signed_duration_since(self.started_at)
            .num_seconds();
        // `num_seconds` can be negative if clocks are weird; treat as 0.
        let elapsed_u64 = u64::try_from(elapsed.max(0)).unwrap_or(u64::MAX);
        elapsed_u64 >= self.max_real_time_seconds
    }

    /// Return the wall-clock start time.
    pub const fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Return elapsed seconds since simulation start.
    pub fn elapsed_seconds(&self) -> u64 {
        let elapsed = Utc::now()
            .signed_duration_since(self.started_at)
            .num_seconds();
        u64::try_from(elapsed.max(0)).unwrap_or(u64::MAX)
    }

    /// Get the configured max ticks.
    pub const fn max_ticks(&self) -> u64 {
        self.max_ticks
    }

    /// Get the configured max real-time seconds.
    pub const fn max_real_time_seconds(&self) -> u64 {
        self.max_real_time_seconds
    }

    // -----------------------------------------------------------------------
    // Event Injection
    // -----------------------------------------------------------------------

    /// Queue an event for injection at the next tick.
    pub async fn inject_event(&self, event: InjectedEvent) {
        let mut queue = self.injected_events.lock().await;
        queue.push(event);
    }

    /// Drain all queued injected events.
    pub async fn drain_injected_events(&self) -> Vec<InjectedEvent> {
        let mut queue = self.injected_events.lock().await;
        std::mem::take(&mut *queue)
    }
}

/// JSON-serializable status of the simulation for the operator API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStatus {
    /// Current tick number.
    pub tick: u64,
    /// Whether the simulation is paused.
    pub paused: bool,
    /// Whether a stop has been requested.
    pub stop_requested: bool,
    /// Current tick interval in milliseconds.
    pub tick_interval_ms: u64,
    /// Elapsed wall-clock seconds since start.
    pub elapsed_seconds: u64,
    /// Configured maximum ticks (0 = unlimited).
    pub max_ticks: u64,
    /// Configured maximum real-time seconds (0 = unlimited).
    pub max_real_time_seconds: u64,
    /// Number of agents currently alive.
    pub agents_alive: u64,
    /// Total agents ever created.
    pub agents_total: u64,
    /// The reason the simulation ended, if applicable.
    pub end_reason: Option<SimulationEndReason>,
    /// ISO 8601 timestamp of when the simulation started.
    pub started_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_bounds() -> SimulationBoundsConfig {
        SimulationBoundsConfig {
            max_ticks: 0,
            max_real_time_seconds: 0,
            end_condition: String::from("manual"),
        }
    }

    #[test]
    fn initial_state_is_not_paused() {
        let state = OperatorState::new(1000, &default_bounds());
        assert!(!state.is_paused());
        assert!(!state.is_stop_requested());
    }

    #[test]
    fn pause_and_resume() {
        let state = OperatorState::new(1000, &default_bounds());
        state.pause();
        assert!(state.is_paused());
        state.resume();
        assert!(!state.is_paused());
    }

    #[test]
    fn stop_request() {
        let state = OperatorState::new(1000, &default_bounds());
        assert!(!state.is_stop_requested());
        state.request_stop();
        assert!(state.is_stop_requested());
    }

    #[test]
    fn set_tick_interval() {
        let state = OperatorState::new(1000, &default_bounds());
        assert_eq!(state.tick_interval_ms(), 1000);
        let prev = state.set_tick_interval_ms(2000);
        assert_eq!(prev, Some(1000));
        assert_eq!(state.tick_interval_ms(), 2000);
    }

    #[test]
    fn reject_sub_100ms_interval() {
        let state = OperatorState::new(1000, &default_bounds());
        let result = state.set_tick_interval_ms(50);
        assert!(result.is_none());
        assert_eq!(state.tick_interval_ms(), 1000);
    }

    #[test]
    fn tick_limit_zero_means_unlimited() {
        let state = OperatorState::new(1000, &default_bounds());
        assert!(!state.tick_limit_reached(999_999));
    }

    #[test]
    fn tick_limit_reached() {
        let bounds = SimulationBoundsConfig {
            max_ticks: 100,
            max_real_time_seconds: 0,
            end_condition: String::from("time_limit"),
        };
        let state = OperatorState::new(1000, &bounds);
        assert!(!state.tick_limit_reached(99));
        assert!(state.tick_limit_reached(100));
        assert!(state.tick_limit_reached(101));
    }

    #[test]
    fn time_limit_zero_means_unlimited() {
        let state = OperatorState::new(1000, &default_bounds());
        assert!(!state.time_limit_reached());
    }

    #[tokio::test]
    async fn inject_and_drain_events() {
        let state = OperatorState::new(1000, &default_bounds());
        state
            .inject_event(InjectedEvent {
                event_type: String::from("plague"),
                target_region: Some(String::from("highlands")),
                severity: None,
                description: None,
            })
            .await;
        let events = state.drain_injected_events().await;
        assert_eq!(events.len(), 1);
        // After drain, queue is empty.
        let events2 = state.drain_injected_events().await;
        assert!(events2.is_empty());
    }
}
