//! Economic system detection for the Emergence simulation.
//!
//! Implements task 6.4.5 from the build plan:
//! - Trade and resource transfer recording
//! - Currency detection (resource appearing in >60% of trades)
//! - Employment detection (repeated resource-for-action patterns)
//! - Taxation detection (regular collection by a leader/group)
//! - Market detection (high trade volume at a location)
//! - Economic model classification (Subsistence, Barter, Market, Command, Feudal)
//! - Wealth distribution analysis (Gini coefficient)
//!
//! # Architecture
//!
//! The economic detector is a **passive analysis layer** that observes
//! trade and transfer events from the simulation. It does not influence
//! agent behavior -- it classifies emergent economic patterns for the
//! observer dashboard and research analytics.
//!
//! All arithmetic uses checked or saturating operations. The Gini
//! coefficient is computed with [`rust_decimal::Decimal`] for precision.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use emergence_types::{AgentId, LocationId, Resource};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Threshold for currency detection: a resource must appear on one side
/// of more than 60% of trades to be considered a currency candidate.
fn currency_threshold() -> Decimal {
    Decimal::new(6, 1)
}

/// Minimum number of trades required before attempting currency detection.
const MIN_TRADES_FOR_CURRENCY: usize = 5;

/// Minimum trades per tick window for a location to be classified as a market.
const MARKET_TRADE_THRESHOLD: u32 = 3;

// ---------------------------------------------------------------------------
// EconomicIndicator
// ---------------------------------------------------------------------------

/// A detected economic phenomenon in the simulation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EconomicIndicator {
    /// Direct goods-for-goods exchange.
    Barter,
    /// A specific resource is being used as a medium of exchange.
    CurrencyAdoption,
    /// One agent regularly compensates another for repeated services.
    Employment,
    /// A leader or group regularly collects resources from members.
    Taxation,
    /// An agent provides resources with expectation of future return.
    Lending,
    /// A location has high trade volume, functioning as a marketplace.
    MarketFormation,
    /// One agent controls a disproportionate share of a resource.
    Monopoly,
    /// Resources are pooled and redistributed among a group.
    Communal,
}

// ---------------------------------------------------------------------------
// EconomicEvent
// ---------------------------------------------------------------------------

/// A recorded economic event for pattern analysis.
#[derive(Debug, Clone)]
pub struct EconomicEvent {
    /// The tick when the event occurred.
    pub tick: u64,
    /// The type of economic phenomenon detected.
    pub indicator: EconomicIndicator,
    /// The agents involved in this event.
    pub agents_involved: Vec<AgentId>,
    /// A human-readable description of the event.
    pub details: String,
}

// ---------------------------------------------------------------------------
// EconomicModel
// ---------------------------------------------------------------------------

/// The overall economic model classification for the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EconomicModel {
    /// No regular trade activity -- agents are self-sufficient.
    Subsistence,
    /// Direct goods-for-goods exchanges only.
    Barter,
    /// A currency has emerged and is used in trade.
    MarketEconomy,
    /// Central collection and redistribution of resources.
    CommandEconomy,
    /// One agent or group controls most resource flows.
    Feudal,
}

// ---------------------------------------------------------------------------
// TradeRecord (internal)
// ---------------------------------------------------------------------------

/// An internal record of a single trade for analysis.
///
/// All fields are stored for future analytics extensions (e.g.,
/// agent-pair trade frequency, resource flow analysis).
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TradeRecord {
    /// The tick when the trade occurred.
    tick: u64,
    /// First trading party.
    agent_a: AgentId,
    /// Second trading party.
    agent_b: AgentId,
    /// Resources given by agent A.
    gave: BTreeMap<Resource, u32>,
    /// Resources received by agent A (given by agent B).
    received: BTreeMap<Resource, u32>,
    /// Location where the trade occurred.
    location: LocationId,
}

/// An internal record of a non-trade resource transfer.
///
/// All fields are stored for future analytics extensions (e.g.,
/// transfer volume by resource type, flow visualization).
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TransferRecord {
    /// The tick when the transfer occurred.
    tick: u64,
    /// The agent giving resources.
    from_agent: AgentId,
    /// The agent receiving resources.
    to_agent: AgentId,
    /// Resources transferred.
    resources: BTreeMap<Resource, u32>,
}

// ---------------------------------------------------------------------------
// EconomicDetector
// ---------------------------------------------------------------------------

/// Detects and classifies emergent economic patterns in the simulation.
///
/// Maintains a history of trades and transfers within a configurable
/// tick window for pattern analysis.
#[derive(Debug, Clone)]
pub struct EconomicDetector {
    /// All recorded trades.
    trades: Vec<TradeRecord>,
    /// All recorded non-trade transfers.
    transfers: Vec<TransferRecord>,
    /// Detected economic events.
    events: Vec<EconomicEvent>,
    /// The size of the analysis window in ticks.
    window_size: u64,
}

impl EconomicDetector {
    /// Create a new economic detector with the given analysis window size.
    ///
    /// `window_size` is the number of ticks to look back when analyzing
    /// patterns. A larger window catches slower-developing patterns but
    /// uses more memory.
    pub const fn new(window_size: u64) -> Self {
        Self {
            trades: Vec::new(),
            transfers: Vec::new(),
            events: Vec::new(),
            window_size,
        }
    }

    /// Record a trade event between two agents.
    ///
    /// `agent_a` gave `gave` and received `received`.
    pub fn record_trade(
        &mut self,
        tick: u64,
        agent_a: AgentId,
        agent_b: AgentId,
        gave: BTreeMap<Resource, u32>,
        received: BTreeMap<Resource, u32>,
        location: LocationId,
    ) {
        self.trades.push(TradeRecord {
            tick,
            agent_a,
            agent_b,
            gave,
            received,
            location,
        });
    }

    /// Record a non-trade resource transfer (gift, tax, tribute).
    pub fn record_resource_transfer(
        &mut self,
        tick: u64,
        from_agent: AgentId,
        to_agent: AgentId,
        resources: BTreeMap<Resource, u32>,
    ) {
        self.transfers.push(TransferRecord {
            tick,
            from_agent,
            to_agent,
            resources,
        });
    }

    /// Detect whether any resource is functioning as a currency.
    ///
    /// A resource is considered a currency candidate if it appears on
    /// one side (either given or received) of more than 60% of all trades
    /// within the analysis window.
    ///
    /// Returns a list of `(Resource, frequency_ratio)` pairs sorted by
    /// frequency, or an empty list if no currency is detected.
    pub fn detect_currency(
        &self,
        current_tick: u64,
    ) -> Result<Vec<(Resource, Decimal)>, AgentError> {
        let window_start = current_tick.saturating_sub(self.window_size);
        let recent_trades: Vec<&TradeRecord> = self
            .trades
            .iter()
            .filter(|t| t.tick >= window_start)
            .collect();

        let trade_count = recent_trades.len();
        if trade_count < MIN_TRADES_FOR_CURRENCY {
            return Ok(Vec::new());
        }

        let trade_count_dec = Decimal::from(trade_count);

        // Count how many trades each resource appears in (on either side)
        let mut resource_counts: BTreeMap<Resource, u32> = BTreeMap::new();

        for trade in &recent_trades {
            let mut resources_in_trade = BTreeSet::new();
            for resource in trade.gave.keys() {
                resources_in_trade.insert(*resource);
            }
            for resource in trade.received.keys() {
                resources_in_trade.insert(*resource);
            }
            for resource in resources_in_trade {
                let count = resource_counts.entry(resource).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        let threshold = currency_threshold();
        let mut candidates = Vec::new();

        for (resource, count) in &resource_counts {
            let count_dec = Decimal::from(*count);
            let ratio = count_dec
                .checked_div(trade_count_dec)
                .ok_or_else(|| AgentError::ArithmeticOverflow {
                    context: String::from("currency detection ratio calculation"),
                })?;

            if ratio > threshold {
                candidates.push((*resource, ratio));
            }
        }

        // Sort by frequency descending
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(candidates)
    }

    /// Detect employment patterns.
    ///
    /// Employment is detected when one agent repeatedly gives resources
    /// to another agent who repeatedly performs actions for them. Here
    /// we approximate this by looking for repeated one-way transfers
    /// from the same source to the same destination.
    ///
    /// Returns pairs of (employer, employee) where the employer has
    /// transferred resources to the employee 3 or more times in the window.
    pub fn detect_employment(&self, current_tick: u64) -> Vec<(AgentId, AgentId)> {
        let window_start = current_tick.saturating_sub(self.window_size);

        let mut transfer_counts: BTreeMap<(AgentId, AgentId), u32> = BTreeMap::new();

        for transfer in &self.transfers {
            if transfer.tick >= window_start {
                let key = (transfer.from_agent, transfer.to_agent);
                let count = transfer_counts.entry(key).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        transfer_counts
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(pair, _)| pair)
            .collect()
    }

    /// Detect taxation patterns.
    ///
    /// Taxation is detected when one agent (a leader) regularly receives
    /// resources from multiple other agents (group members). We look for
    /// agents receiving transfers from 3 or more distinct agents in the window.
    ///
    /// Returns a list of (collector, payers) where the collector received
    /// from at least 3 distinct payers.
    pub fn detect_taxation(
        &self,
        current_tick: u64,
    ) -> Vec<(AgentId, BTreeSet<AgentId>)> {
        let window_start = current_tick.saturating_sub(self.window_size);

        let mut collector_to_payers: BTreeMap<AgentId, BTreeSet<AgentId>> = BTreeMap::new();

        for transfer in &self.transfers {
            if transfer.tick >= window_start {
                collector_to_payers
                    .entry(transfer.to_agent)
                    .or_default()
                    .insert(transfer.from_agent);
            }
        }

        collector_to_payers
            .into_iter()
            .filter(|(_, payers)| payers.len() >= 3)
            .collect()
    }

    /// Detect market locations.
    ///
    /// A location is classified as a market if it sees more than
    /// `MARKET_TRADE_THRESHOLD` trades within the analysis window.
    ///
    /// Returns a list of `(LocationId, trade_count)` pairs.
    pub fn detect_market(&self, current_tick: u64) -> Vec<(LocationId, u32)> {
        let window_start = current_tick.saturating_sub(self.window_size);

        let mut location_counts: BTreeMap<LocationId, u32> = BTreeMap::new();

        for trade in &self.trades {
            if trade.tick >= window_start {
                let count = location_counts.entry(trade.location).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        location_counts
            .into_iter()
            .filter(|(_, count)| *count >= MARKET_TRADE_THRESHOLD)
            .collect()
    }

    /// Classify the overall economic model based on detected patterns.
    ///
    /// Classification logic:
    /// - No regular trade activity in the window: `Subsistence`
    /// - Direct exchanges only (no currency): `Barter`
    /// - Currency present: `MarketEconomy`
    /// - Central collection + redistribution detected: `CommandEconomy`
    /// - One agent controls most resource flows: `Feudal`
    pub fn classify_economic_model(
        &self,
        current_tick: u64,
    ) -> Result<EconomicModel, AgentError> {
        let window_start = current_tick.saturating_sub(self.window_size);

        let recent_trade_count = self
            .trades
            .iter()
            .filter(|t| t.tick >= window_start)
            .count();

        if recent_trade_count == 0 {
            return Ok(EconomicModel::Subsistence);
        }

        // Check for taxation (command economy signal)
        let taxation = self.detect_taxation(current_tick);
        if !taxation.is_empty() {
            // Check if a single collector dominates
            let max_payer_count = taxation
                .iter()
                .map(|(_, payers)| payers.len())
                .max()
                .unwrap_or(0);

            if max_payer_count >= 5 {
                return Ok(EconomicModel::Feudal);
            }
            return Ok(EconomicModel::CommandEconomy);
        }

        // Check for currency
        let currency_candidates = self.detect_currency(current_tick)?;
        if !currency_candidates.is_empty() {
            return Ok(EconomicModel::MarketEconomy);
        }

        Ok(EconomicModel::Barter)
    }

    /// Compute the Gini coefficient for wealth distribution.
    ///
    /// Takes a map from agent ID to total wealth (sum of all resources).
    /// Returns a value between 0.0 (perfect equality) and 1.0 (maximum
    /// inequality).
    ///
    /// Uses the standard formula:
    /// `G = (sum of |xi - xj| for all i,j) / (2 * n * sum of xi)`
    pub fn get_wealth_distribution(
        &self,
        agent_wealth: &BTreeMap<AgentId, u32>,
    ) -> Result<Decimal, AgentError> {
        let n = agent_wealth.len();
        if n == 0 {
            return Ok(Decimal::ZERO);
        }

        let values: Vec<u32> = agent_wealth.values().copied().collect();

        let mut sum_abs_diff: u64 = 0;
        let mut total_wealth: u64 = 0;

        for val in &values {
            total_wealth = total_wealth.saturating_add(u64::from(*val));
        }

        if total_wealth == 0 {
            return Ok(Decimal::ZERO);
        }

        // Compute sum of absolute differences
        for (i, vi) in values.iter().enumerate() {
            for vj in values.iter().skip(i.saturating_add(1)) {
                let diff = if *vi >= *vj {
                    u64::from(vi.saturating_sub(*vj))
                } else {
                    u64::from(vj.saturating_sub(*vi))
                };
                // Each pair is counted twice in the full formula, so multiply by 2
                sum_abs_diff = sum_abs_diff.saturating_add(diff.saturating_mul(2));
            }
        }

        let n_dec = Decimal::from(n as u64);
        let sum_diff_dec = Decimal::from(sum_abs_diff);
        let total_dec = Decimal::from(total_wealth);

        let two = Decimal::from(2_u64);

        let denominator = two
            .checked_mul(n_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("Gini denominator 2*n overflow"),
            })?
            .checked_mul(total_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("Gini denominator 2*n*sum overflow"),
            })?;

        if denominator == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        sum_diff_dec
            .checked_div(denominator)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("Gini coefficient division overflow"),
            })
    }

    /// Get trade volume (number of trades) per tick over the analysis window.
    ///
    /// Returns a map from tick to number of trades that occurred on that tick.
    pub fn get_trade_volume(&self, current_tick: u64) -> BTreeMap<u64, u32> {
        let window_start = current_tick.saturating_sub(self.window_size);
        let mut volume: BTreeMap<u64, u32> = BTreeMap::new();

        for trade in &self.trades {
            if trade.tick >= window_start {
                let count = volume.entry(trade.tick).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        volume
    }

    /// Get resources that might be functioning as currency.
    ///
    /// Returns all resources that appear in more than 40% of recent trades
    /// (a lower threshold than `detect_currency` for exploratory analysis).
    pub fn get_currency_candidates(
        &self,
        current_tick: u64,
    ) -> Result<Vec<(Resource, Decimal)>, AgentError> {
        let window_start = current_tick.saturating_sub(self.window_size);
        let recent_trades: Vec<&TradeRecord> = self
            .trades
            .iter()
            .filter(|t| t.tick >= window_start)
            .collect();

        let trade_count = recent_trades.len();
        if trade_count < 3 {
            return Ok(Vec::new());
        }

        let trade_count_dec = Decimal::from(trade_count);
        let candidate_threshold = Decimal::new(4, 1); // 0.4 (40%)

        let mut resource_counts: BTreeMap<Resource, u32> = BTreeMap::new();

        for trade in &recent_trades {
            let mut resources_in_trade = BTreeSet::new();
            for resource in trade.gave.keys() {
                resources_in_trade.insert(*resource);
            }
            for resource in trade.received.keys() {
                resources_in_trade.insert(*resource);
            }
            for resource in resources_in_trade {
                let count = resource_counts.entry(resource).or_insert(0);
                *count = count.saturating_add(1);
            }
        }

        let mut candidates = Vec::new();

        for (resource, count) in &resource_counts {
            let count_dec = Decimal::from(*count);
            let ratio = count_dec
                .checked_div(trade_count_dec)
                .ok_or_else(|| AgentError::ArithmeticOverflow {
                    context: String::from("currency candidate ratio calculation"),
                })?;

            if ratio > candidate_threshold {
                candidates.push((*resource, ratio));
            }
        }

        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(candidates)
    }

    /// Get the total number of trades recorded.
    pub const fn total_trades(&self) -> usize {
        self.trades.len()
    }

    /// Get the total number of non-trade transfers recorded.
    pub const fn total_transfers(&self) -> usize {
        self.transfers.len()
    }

    /// Get all detected economic events.
    pub fn events(&self) -> &[EconomicEvent] {
        &self.events
    }

    /// Record a detected economic event.
    pub fn record_event(&mut self, event: EconomicEvent) {
        self.events.push(event);
    }
}

impl Default for EconomicDetector {
    fn default() -> Self {
        Self::new(100)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trade(
        detector: &mut EconomicDetector,
        tick: u64,
        agent_a: AgentId,
        agent_b: AgentId,
        gave_resource: Resource,
        gave_qty: u32,
        received_resource: Resource,
        received_qty: u32,
        location: LocationId,
    ) {
        let mut gave = BTreeMap::new();
        gave.insert(gave_resource, gave_qty);
        let mut received = BTreeMap::new();
        received.insert(received_resource, received_qty);
        detector.record_trade(tick, agent_a, agent_b, gave, received, location);
    }

    // -----------------------------------------------------------------------
    // Trade recording
    // -----------------------------------------------------------------------

    #[test]
    fn record_trade_increments_count() {
        let mut detector = EconomicDetector::new(100);
        let a = AgentId::new();
        let b = AgentId::new();
        let loc = LocationId::new();

        make_trade(&mut detector, 1, a, b, Resource::Wood, 5, Resource::Stone, 3, loc);
        assert_eq!(detector.total_trades(), 1);
    }

    #[test]
    fn record_transfer_increments_count() {
        let mut detector = EconomicDetector::new(100);
        let a = AgentId::new();
        let b = AgentId::new();

        let mut resources = BTreeMap::new();
        resources.insert(Resource::FoodBerry, 10);
        detector.record_resource_transfer(1, a, b, resources);

        assert_eq!(detector.total_transfers(), 1);
    }

    // -----------------------------------------------------------------------
    // Currency detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_currency_insufficient_trades() {
        let detector = EconomicDetector::new(100);
        let result = detector.detect_currency(50);
        assert!(result.is_ok());
        assert!(result.unwrap_or_default().is_empty());
    }

    #[test]
    fn detect_currency_single_dominant_resource() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // Create 10 trades where CurrencyToken appears in 8 of them
        for i in 0_u64..8 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(
                &mut detector,
                i,
                a,
                b,
                Resource::CurrencyToken,
                5,
                Resource::Wood,
                3,
                loc,
            );
        }
        // 2 trades without CurrencyToken
        for i in 8_u64..10 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(
                &mut detector,
                i,
                a,
                b,
                Resource::Stone,
                5,
                Resource::FoodBerry,
                3,
                loc,
            );
        }

        let result = detector.detect_currency(20);
        assert!(result.is_ok());

        let candidates = result.unwrap_or_default();
        // CurrencyToken appears in 8/10 = 80% > 60%
        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|(r, _)| *r == Resource::CurrencyToken));
    }

    #[test]
    fn detect_currency_no_dominant_resource() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // Create diverse trades with no dominant resource
        let resources = [
            Resource::Wood,
            Resource::Stone,
            Resource::FoodBerry,
            Resource::FoodFish,
            Resource::Water,
        ];

        for (i, pair) in resources.windows(2).enumerate() {
            if let (Some(r1), Some(r2)) = (pair.first(), pair.get(1)) {
                let a = AgentId::new();
                let b = AgentId::new();
                make_trade(
                    &mut detector,
                    i as u64,
                    a,
                    b,
                    *r1,
                    5,
                    *r2,
                    3,
                    loc,
                );
            }
        }

        // Add more trades to reach minimum
        for i in 4_u64..8 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(
                &mut detector,
                i,
                a,
                b,
                Resource::Fiber,
                2,
                Resource::Clay,
                4,
                loc,
            );
        }

        let result = detector.detect_currency(20);
        assert!(result.is_ok());
        let candidates = result.unwrap_or_default();
        // No resource should appear in >60% of the diverse trades
        assert!(candidates.is_empty());
    }

    // -----------------------------------------------------------------------
    // Market detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_market_high_volume_location() {
        let mut detector = EconomicDetector::new(100);
        let market_loc = LocationId::new();
        let quiet_loc = LocationId::new();

        // 5 trades at market_loc
        for i in 0_u64..5 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::Wood, 5, Resource::Stone, 3, market_loc);
        }

        // 1 trade at quiet_loc
        let a = AgentId::new();
        let b = AgentId::new();
        make_trade(&mut detector, 1, a, b, Resource::Wood, 5, Resource::Stone, 3, quiet_loc);

        let markets = detector.detect_market(20);
        assert_eq!(markets.len(), 1);
        assert!(markets.first().is_some_and(|(loc, _)| *loc == market_loc));
    }

    #[test]
    fn detect_market_no_markets() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // Only 2 trades at this location -- below threshold
        for i in 0_u64..2 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::Wood, 5, Resource::Stone, 3, loc);
        }

        let markets = detector.detect_market(20);
        assert!(markets.is_empty());
    }

    // -----------------------------------------------------------------------
    // Employment detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_employment_repeated_transfers() {
        let mut detector = EconomicDetector::new(100);
        let employer = AgentId::new();
        let employee = AgentId::new();

        // Employer pays employee 4 times
        for i in 0_u64..4 {
            let mut resources = BTreeMap::new();
            resources.insert(Resource::FoodBerry, 5);
            detector.record_resource_transfer(i, employer, employee, resources);
        }

        let employment = detector.detect_employment(20);
        assert_eq!(employment.len(), 1);
        assert!(employment.first().is_some_and(|(emp, wrk)| *emp == employer && *wrk == employee));
    }

    #[test]
    fn detect_employment_insufficient_transfers() {
        let mut detector = EconomicDetector::new(100);
        let employer = AgentId::new();
        let employee = AgentId::new();

        // Only 2 transfers -- below threshold of 3
        for i in 0_u64..2 {
            let mut resources = BTreeMap::new();
            resources.insert(Resource::FoodBerry, 5);
            detector.record_resource_transfer(i, employer, employee, resources);
        }

        let employment = detector.detect_employment(20);
        assert!(employment.is_empty());
    }

    // -----------------------------------------------------------------------
    // Taxation detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_taxation_multiple_payers() {
        let mut detector = EconomicDetector::new(100);
        let collector = AgentId::new();

        // 4 different agents pay the collector
        for i in 0_u64..4 {
            let payer = AgentId::new();
            let mut resources = BTreeMap::new();
            resources.insert(Resource::FoodBerry, 3);
            detector.record_resource_transfer(i, payer, collector, resources);
        }

        let taxation = detector.detect_taxation(20);
        assert_eq!(taxation.len(), 1);
        assert!(taxation.first().is_some_and(|(c, payers)| *c == collector && payers.len() == 4));
    }

    #[test]
    fn detect_taxation_insufficient_payers() {
        let mut detector = EconomicDetector::new(100);
        let collector = AgentId::new();

        // Only 2 payers -- below threshold of 3
        for i in 0_u64..2 {
            let payer = AgentId::new();
            let mut resources = BTreeMap::new();
            resources.insert(Resource::FoodBerry, 3);
            detector.record_resource_transfer(i, payer, collector, resources);
        }

        let taxation = detector.detect_taxation(20);
        assert!(taxation.is_empty());
    }

    // -----------------------------------------------------------------------
    // Economic model classification
    // -----------------------------------------------------------------------

    #[test]
    fn classify_subsistence_no_trades() {
        let detector = EconomicDetector::new(100);
        let result = detector.classify_economic_model(50);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(EconomicModel::Barter), EconomicModel::Subsistence);
    }

    #[test]
    fn classify_barter_economy() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // A few diverse trades, no currency
        for i in 0_u64..3 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::Wood, 5, Resource::Stone, 3, loc);
        }

        let result = detector.classify_economic_model(20);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(EconomicModel::Subsistence), EconomicModel::Barter);
    }

    #[test]
    fn classify_market_economy_with_currency() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // 8 trades with CurrencyToken, 2 without -- triggers currency detection
        for i in 0_u64..8 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::CurrencyToken, 5, Resource::Wood, 3, loc);
        }
        for i in 8_u64..10 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::Stone, 5, Resource::FoodBerry, 3, loc);
        }

        let result = detector.classify_economic_model(20);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(EconomicModel::Subsistence), EconomicModel::MarketEconomy);
    }

    // -----------------------------------------------------------------------
    // Gini coefficient
    // -----------------------------------------------------------------------

    #[test]
    fn gini_perfect_equality() {
        let detector = EconomicDetector::new(100);
        let mut wealth = BTreeMap::new();
        wealth.insert(AgentId::new(), 100);
        wealth.insert(AgentId::new(), 100);
        wealth.insert(AgentId::new(), 100);

        let result = detector.get_wealth_distribution(&wealth);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    #[test]
    fn gini_maximum_inequality() {
        let detector = EconomicDetector::new(100);
        let mut wealth = BTreeMap::new();
        wealth.insert(AgentId::new(), 300);
        wealth.insert(AgentId::new(), 0);
        wealth.insert(AgentId::new(), 0);

        let result = detector.get_wealth_distribution(&wealth);
        assert!(result.is_ok());

        // For 3 agents: [0, 0, 300]
        // sum_abs_diff = 2*(|300-0| + |300-0| + |0-0|) = 2 * 600 = 1200
        // denominator = 2 * 3 * 300 = 1800
        // Gini = 1200 / 1800 = 0.666...
        let gini = result.unwrap_or(Decimal::ZERO);
        // Should be approximately 2/3
        let two_thirds = Decimal::new(2, 0)
            .checked_div(Decimal::new(3, 0))
            .unwrap_or(Decimal::ZERO);
        let diff = if gini > two_thirds {
            gini.saturating_sub(two_thirds)
        } else {
            two_thirds.saturating_sub(gini)
        };
        assert!(diff < Decimal::new(1, 2)); // within 0.01
    }

    #[test]
    fn gini_empty_population() {
        let detector = EconomicDetector::new(100);
        let wealth: BTreeMap<AgentId, u32> = BTreeMap::new();

        let result = detector.get_wealth_distribution(&wealth);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    #[test]
    fn gini_zero_wealth() {
        let detector = EconomicDetector::new(100);
        let mut wealth = BTreeMap::new();
        wealth.insert(AgentId::new(), 0);
        wealth.insert(AgentId::new(), 0);

        let result = detector.get_wealth_distribution(&wealth);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Trade volume
    // -----------------------------------------------------------------------

    #[test]
    fn trade_volume_per_tick() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // 3 trades on tick 5, 2 on tick 10
        for _ in 0..3 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, 5, a, b, Resource::Wood, 5, Resource::Stone, 3, loc);
        }
        for _ in 0..2 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, 10, a, b, Resource::Wood, 5, Resource::Stone, 3, loc);
        }

        let volume = detector.get_trade_volume(20);
        assert_eq!(volume.get(&5), Some(&3));
        assert_eq!(volume.get(&10), Some(&2));
    }

    // -----------------------------------------------------------------------
    // Currency candidates (lower threshold)
    // -----------------------------------------------------------------------

    #[test]
    fn currency_candidates_lower_threshold() {
        let mut detector = EconomicDetector::new(100);
        let loc = LocationId::new();

        // CurrencyToken in 3/6 = 50% > 40% threshold
        for i in 0_u64..3 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::CurrencyToken, 5, Resource::Wood, 3, loc);
        }
        for i in 3_u64..6 {
            let a = AgentId::new();
            let b = AgentId::new();
            make_trade(&mut detector, i, a, b, Resource::Stone, 5, Resource::FoodBerry, 3, loc);
        }

        let result = detector.get_currency_candidates(20);
        assert!(result.is_ok());
        let candidates = result.unwrap_or_default();
        assert!(candidates.iter().any(|(r, _)| *r == Resource::CurrencyToken));
    }
}
