//! LLM cost tracking for the agent runner.
//!
//! Provides a thread-safe [`CostTracker`] that records token usage per LLM
//! call and computes estimated costs using configurable per-million-token
//! rates. Costs are tracked separately for the primary (default) and
//! escalation backends.
//!
//! All monetary calculations use [`rust_decimal::Decimal`] for financial
//! precision -- no floating-point arithmetic.

use std::fmt;
use std::sync::Mutex;

use rust_decimal::Decimal;

/// One million, used as the denominator for per-million-token pricing.
///
/// Stored as a constant to avoid repeated construction.
const ONE_MILLION: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);

/// Thread-safe LLM cost tracker.
///
/// Holds per-million-token pricing for two backend tiers (primary and
/// escalation) and accumulates token counts and estimated costs across
/// all recorded calls. Safe to share via `Arc<CostTracker>`.
///
/// # Usage
///
/// ```text
/// let tracker = CostTracker::new(
///     Decimal::new(30, 2),   // $0.30 per 1M input (primary)
///     Decimal::new(88, 2),   // $0.88 per 1M output (primary)
///     Decimal::new(300, 2),  // $3.00 per 1M input (escalation)
///     Decimal::new(1500, 2), // $15.00 per 1M output (escalation)
/// );
///
/// tracker.record_call("primary", 1000, 200);
/// let summary = tracker.summary();
/// assert_eq!(summary.total_calls, 1);
/// ```
pub struct CostTracker {
    /// Price per million input tokens for the primary backend.
    primary_input_rate: Decimal,
    /// Price per million output tokens for the primary backend.
    primary_output_rate: Decimal,
    /// Price per million input tokens for the escalation backend.
    escalation_input_rate: Decimal,
    /// Price per million output tokens for the escalation backend.
    escalation_output_rate: Decimal,
    /// Mutable interior state protected by a mutex.
    inner: Mutex<CostTrackerInner>,
}

/// Mutable accumulation state held inside the mutex.
#[derive(Debug, Default)]
struct CostTrackerInner {
    /// Total number of LLM calls recorded.
    total_calls: u64,
    /// Total input tokens across all calls.
    total_input_tokens: u64,
    /// Total output tokens across all calls.
    total_output_tokens: u64,
    /// Running estimated cost in dollars.
    total_estimated_cost: Decimal,
    /// Number of calls routed to the primary backend.
    primary_calls: u64,
    /// Number of calls routed to the escalation backend.
    escalation_calls: u64,
}

/// Snapshot of cost tracking state returned by [`CostTracker::summary`].
///
/// Not yet consumed outside tests; will be used by shutdown hooks and
/// the operator API once those are wired up.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CostSummary {
    /// Total number of LLM calls recorded.
    pub total_calls: u64,
    /// Total input tokens across all calls.
    pub total_input_tokens: u64,
    /// Total output tokens across all calls.
    pub total_output_tokens: u64,
    /// Running estimated cost in dollars.
    pub total_estimated_cost: Decimal,
    /// Number of calls routed to the primary backend.
    pub primary_calls: u64,
    /// Number of calls routed to the escalation backend.
    pub escalation_calls: u64,
}

impl CostTracker {
    /// Create a new cost tracker with per-million-token pricing.
    ///
    /// Rates are in dollars per million tokens. For example, `Decimal::new(30, 2)`
    /// represents $0.30 per million tokens.
    pub const fn new(
        primary_input_rate: Decimal,
        primary_output_rate: Decimal,
        escalation_input_rate: Decimal,
        escalation_output_rate: Decimal,
    ) -> Self {
        Self {
            primary_input_rate,
            primary_output_rate,
            escalation_input_rate,
            escalation_output_rate,
            inner: Mutex::new(CostTrackerInner {
                total_calls: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_estimated_cost: Decimal::ZERO,
                primary_calls: 0,
                escalation_calls: 0,
            }),
        }
    }

    /// Record a completed LLM call with token usage.
    ///
    /// `backend_label` should be `"primary"` or `"escalation"` to select
    /// the correct pricing tier. Any other label is treated as primary
    /// pricing.
    ///
    /// Token counts that would overflow the running totals are clamped
    /// via saturating addition.
    pub fn record_call(&self, backend_label: &str, input_tokens: u64, output_tokens: u64) {
        let is_escalation = backend_label == "escalation";

        let (input_rate, output_rate) = if is_escalation {
            (self.escalation_input_rate, self.escalation_output_rate)
        } else {
            (self.primary_input_rate, self.primary_output_rate)
        };

        let input_dec = Decimal::from(input_tokens);
        let output_dec = Decimal::from(output_tokens);

        // cost = (input_tokens / 1_000_000) * input_rate
        //      + (output_tokens / 1_000_000) * output_rate
        //
        // Division and multiplication on Decimal do not overflow; they
        // produce exact results within Decimal's 96-bit mantissa.
        let input_cost = input_dec
            .checked_div(ONE_MILLION)
            .unwrap_or(Decimal::ZERO)
            .checked_mul(input_rate)
            .unwrap_or(Decimal::ZERO);
        let output_cost = output_dec
            .checked_div(ONE_MILLION)
            .unwrap_or(Decimal::ZERO)
            .checked_mul(output_rate)
            .unwrap_or(Decimal::ZERO);
        let call_cost = input_cost
            .checked_add(output_cost)
            .unwrap_or(Decimal::ZERO);

        // Lock the inner state and update accumulators.
        // If the mutex is poisoned we still try to recover -- worst case
        // we silently skip the update rather than panicking.
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        inner.total_calls = inner.total_calls.saturating_add(1);
        inner.total_input_tokens = inner.total_input_tokens.saturating_add(input_tokens);
        inner.total_output_tokens = inner.total_output_tokens.saturating_add(output_tokens);
        inner.total_estimated_cost = inner
            .total_estimated_cost
            .checked_add(call_cost)
            .unwrap_or(inner.total_estimated_cost);

        if is_escalation {
            inner.escalation_calls = inner.escalation_calls.saturating_add(1);
        } else {
            inner.primary_calls = inner.primary_calls.saturating_add(1);
        }
    }

    /// Return a snapshot of the current cost tracking state.
    ///
    /// Returns a zeroed summary if the mutex is poisoned.
    ///
    /// Not yet consumed outside tests; will be used by shutdown hooks and
    /// the operator API once those are wired up.
    #[allow(dead_code)]
    pub fn summary(&self) -> CostSummary {
        let Ok(inner) = self.inner.lock() else {
            return CostSummary {
                total_calls: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_estimated_cost: Decimal::ZERO,
                primary_calls: 0,
                escalation_calls: 0,
            };
        };

        CostSummary {
            total_calls: inner.total_calls,
            total_input_tokens: inner.total_input_tokens,
            total_output_tokens: inner.total_output_tokens,
            total_estimated_cost: inner.total_estimated_cost,
            primary_calls: inner.primary_calls,
            escalation_calls: inner.escalation_calls,
        }
    }
}

impl fmt::Display for CostSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LLM Cost Summary: {} calls ({} primary, {} escalation) | \
             {} input tokens, {} output tokens | \
             estimated cost: ${}",
            self.total_calls,
            self.primary_calls,
            self.escalation_calls,
            self.total_input_tokens,
            self.total_output_tokens,
            self.total_estimated_cost,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tracker() -> CostTracker {
        CostTracker::new(
            Decimal::new(30, 2),   // $0.30 per 1M input (primary)
            Decimal::new(88, 2),   // $0.88 per 1M output (primary)
            Decimal::new(300, 2),  // $3.00 per 1M input (escalation)
            Decimal::new(1500, 2), // $15.00 per 1M output (escalation)
        )
    }

    #[test]
    fn record_single_primary_call() {
        let tracker = test_tracker();
        tracker.record_call("primary", 1_000_000, 1_000_000);
        let summary = tracker.summary();

        assert_eq!(summary.total_calls, 1);
        assert_eq!(summary.primary_calls, 1);
        assert_eq!(summary.escalation_calls, 0);
        assert_eq!(summary.total_input_tokens, 1_000_000);
        assert_eq!(summary.total_output_tokens, 1_000_000);

        // Cost = (1M / 1M) * $0.30 + (1M / 1M) * $0.88 = $1.18
        let expected = Decimal::new(118, 2);
        assert_eq!(summary.total_estimated_cost, expected);
    }

    #[test]
    fn record_single_escalation_call() {
        let tracker = test_tracker();
        tracker.record_call("escalation", 500_000, 100_000);
        let summary = tracker.summary();

        assert_eq!(summary.total_calls, 1);
        assert_eq!(summary.primary_calls, 0);
        assert_eq!(summary.escalation_calls, 1);
        assert_eq!(summary.total_input_tokens, 500_000);
        assert_eq!(summary.total_output_tokens, 100_000);

        // Cost = (500k / 1M) * $3.00 + (100k / 1M) * $15.00
        //      = 0.5 * 3.00 + 0.1 * 15.00
        //      = 1.50 + 1.50 = $3.00
        let expected = Decimal::new(300, 2);
        assert_eq!(summary.total_estimated_cost, expected);
    }

    #[test]
    fn record_multiple_calls_accumulates() {
        let tracker = test_tracker();
        tracker.record_call("primary", 1000, 200);
        tracker.record_call("primary", 1000, 200);
        tracker.record_call("escalation", 500, 100);
        let summary = tracker.summary();

        assert_eq!(summary.total_calls, 3);
        assert_eq!(summary.primary_calls, 2);
        assert_eq!(summary.escalation_calls, 1);
        assert_eq!(summary.total_input_tokens, 2500);
        assert_eq!(summary.total_output_tokens, 500);
    }

    #[test]
    fn zero_tokens_records_zero_cost() {
        let tracker = test_tracker();
        tracker.record_call("primary", 0, 0);
        let summary = tracker.summary();

        assert_eq!(summary.total_calls, 1);
        assert_eq!(summary.total_estimated_cost, Decimal::ZERO);
    }

    #[test]
    fn unknown_label_uses_primary_rates() {
        let tracker = test_tracker();
        tracker.record_call("unknown-backend", 1_000_000, 0);
        let summary = tracker.summary();

        assert_eq!(summary.primary_calls, 1);
        assert_eq!(summary.escalation_calls, 0);
        // Should use primary input rate: $0.30
        assert_eq!(summary.total_estimated_cost, Decimal::new(30, 2));
    }

    #[test]
    fn summary_empty_tracker() {
        let tracker = test_tracker();
        let summary = tracker.summary();

        assert_eq!(summary.total_calls, 0);
        assert_eq!(summary.primary_calls, 0);
        assert_eq!(summary.escalation_calls, 0);
        assert_eq!(summary.total_input_tokens, 0);
        assert_eq!(summary.total_output_tokens, 0);
        assert_eq!(summary.total_estimated_cost, Decimal::ZERO);
    }

    #[test]
    fn summary_display_format() {
        let tracker = test_tracker();
        tracker.record_call("primary", 1000, 200);
        let summary = tracker.summary();
        let display = format!("{summary}");

        assert!(display.contains("1 calls"));
        assert!(display.contains("1 primary"));
        assert!(display.contains("0 escalation"));
        assert!(display.contains("1000 input tokens"));
        assert!(display.contains("200 output tokens"));
        assert!(display.contains("estimated cost: $"));
    }

    #[test]
    fn thread_safety_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(test_tracker());
        let mut handles = Vec::new();

        for _ in 0..10 {
            let t = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    t.record_call("primary", 1000, 200);
                }
            }));
        }

        for handle in handles {
            handle.join().ok();
        }

        let summary = tracker.summary();
        assert_eq!(summary.total_calls, 1000);
        assert_eq!(summary.primary_calls, 1000);
        assert_eq!(summary.total_input_tokens, 1_000_000);
        assert_eq!(summary.total_output_tokens, 200_000);
    }
}
