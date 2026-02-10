//! Crime and justice tracking for the Emergence simulation.
//!
//! Implements task 6.4.6 from the build plan:
//! - Crime recording (theft, assault, murder, deception, trespass, rule violations)
//! - Punishment tracking (exile, confiscation, shaming, imprisonment, restitution)
//! - Policing action recording
//! - Crime rate, detection rate, punishment rate, recidivism rate
//! - Justice system classification (no justice, self-policing, vigilante,
//!   centralized, court system)
//! - Hotspot analysis (crime by location)
//! - Serial offender detection
//!
//! # Architecture
//!
//! The crime tracker is a **passive observation layer** that records
//! criminal events as they occur during the simulation. It classifies
//! the emergent justice patterns based on who punishes whom, how often,
//! and whether formal processes (voting) precede punishment.
//!
//! All rate calculations use checked arithmetic and return [`Decimal`]
//! for precision.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;
use uuid::Uuid;

use emergence_types::{AgentId, LocationId};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// CrimeType
// ---------------------------------------------------------------------------

/// The category of crime committed by an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrimeType {
    /// Taking resources from another agent without consent.
    Theft,
    /// Physical attack on another agent.
    Assault,
    /// Killing another agent.
    Murder,
    /// Communicating false information with intent to deceive.
    Deception,
    /// Entering a restricted area without permission.
    Trespass,
    /// Violating a governance rule established by a group.
    RuleViolation,
}

// ---------------------------------------------------------------------------
// PunishmentType
// ---------------------------------------------------------------------------

/// The type of punishment applied to a criminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PunishmentType {
    /// Forced removal from a location or group.
    Exile,
    /// Taking resources from the offender.
    ResourceConfiscation,
    /// Physical harm dealt as punishment.
    PhysicalPunishment,
    /// Public announcement of the crime, damaging reputation.
    SocialShaming,
    /// Confinement to a location.
    Imprisonment,
    /// Requiring the offender to compensate the victim.
    Restitution,
}

// ---------------------------------------------------------------------------
// JusticePattern
// ---------------------------------------------------------------------------

/// The overall justice system pattern detected in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JusticePattern {
    /// No punishments are being applied to crimes.
    NoJustice,
    /// Victims punish their own offenders directly.
    SelfPolicing,
    /// Random (non-designated) agents punish offenders.
    VigilanteJustice,
    /// A small set of designated agents do most punishing.
    CentralizedPolicing,
    /// Voting or trial processes precede punishment decisions.
    CourtSystem,
}

// ---------------------------------------------------------------------------
// CrimeRecord
// ---------------------------------------------------------------------------

/// A record of a crime committed during the simulation.
#[derive(Debug, Clone)]
pub struct CrimeRecord {
    /// Unique identifier for this crime record.
    pub id: Uuid,
    /// The tick when the crime occurred.
    pub tick: u64,
    /// The category of crime.
    pub crime_type: CrimeType,
    /// The agent who committed the crime.
    pub perpetrator: AgentId,
    /// The victim of the crime, if any (e.g., trespass may have no specific victim).
    pub victim: Option<AgentId>,
    /// The location where the crime occurred.
    pub location: Option<LocationId>,
    /// Whether the crime was detected by someone.
    pub detected: bool,
    /// Whether the perpetrator has been punished for this crime.
    pub punished: bool,
}

// ---------------------------------------------------------------------------
// PunishmentRecord
// ---------------------------------------------------------------------------

/// A record of punishment applied for a crime.
#[derive(Debug, Clone)]
pub struct PunishmentRecord {
    /// The crime this punishment is for.
    pub crime_id: Uuid,
    /// The agent who administered the punishment.
    pub punished_by: AgentId,
    /// The tick when the punishment occurred.
    pub tick: u64,
    /// The type of punishment applied.
    pub punishment_type: PunishmentType,
    /// A human-readable description of the punishment.
    pub details: String,
}

// ---------------------------------------------------------------------------
// PolicingAction (internal tracking)
// ---------------------------------------------------------------------------

/// A record of a policing action (patrol, investigation, enforcement).
///
/// Fields are stored for historical queries and future analytics
/// extensions (e.g., policing frequency per agent, patrol routes).
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PolicingAction {
    /// The agent performing the policing action.
    agent_id: AgentId,
    /// The tick when the policing action occurred.
    tick: u64,
}

// ---------------------------------------------------------------------------
// CrimeTracker
// ---------------------------------------------------------------------------

/// Tracks all crimes, punishments, and justice patterns in the simulation.
///
/// Provides methods to record events and compute analytics like crime rates,
/// detection rates, recidivism rates, and justice system classification.
#[derive(Debug, Clone)]
pub struct CrimeTracker {
    /// All crime records, keyed by their unique ID.
    crimes: BTreeMap<Uuid, CrimeRecord>,
    /// All punishment records, keyed by the crime ID they address.
    punishments: BTreeMap<Uuid, Vec<PunishmentRecord>>,
    /// Policing actions for authority detection.
    policing_actions: Vec<PolicingAction>,
    /// Per-agent crime count for serial offender detection.
    agent_crime_count: BTreeMap<AgentId, u32>,
    /// Per-agent punishment count for justice pattern analysis.
    agent_punishment_count: BTreeMap<AgentId, u32>,
    /// Set of agents known to have voted on punishment decisions
    /// (signal for court system detection).
    punishment_voters: BTreeSet<AgentId>,
}

impl CrimeTracker {
    /// Create a new empty crime tracker.
    pub const fn new() -> Self {
        Self {
            crimes: BTreeMap::new(),
            punishments: BTreeMap::new(),
            policing_actions: Vec::new(),
            agent_crime_count: BTreeMap::new(),
            agent_punishment_count: BTreeMap::new(),
            punishment_voters: BTreeSet::new(),
        }
    }

    /// Record a crime event.
    ///
    /// Returns the unique ID assigned to this crime record.
    pub fn record_crime(&mut self, record: CrimeRecord) -> Uuid {
        let id = record.id;
        let perpetrator = record.perpetrator;
        self.crimes.insert(id, record);

        let count = self.agent_crime_count.entry(perpetrator).or_insert(0);
        *count = count.saturating_add(1);

        id
    }

    /// Record a punishment for a crime.
    ///
    /// Marks the crime as punished and records the punishment details.
    /// Returns an error if the crime ID is not found.
    pub fn record_punishment(
        &mut self,
        punishment: PunishmentRecord,
    ) -> Result<(), AgentError> {
        let crime_id = punishment.crime_id;
        let punisher = punishment.punished_by;

        // Mark the crime as punished
        let crime = self.crimes.get_mut(&crime_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("crime {crime_id} not found for punishment"),
            }
        })?;
        crime.punished = true;

        // Record the punishment
        self.punishments
            .entry(crime_id)
            .or_default()
            .push(punishment);

        // Track punisher activity
        let count = self.agent_punishment_count.entry(punisher).or_insert(0);
        *count = count.saturating_add(1);

        Ok(())
    }

    /// Record a policing action (patrol, investigation, enforcement).
    pub fn record_policing_action(&mut self, agent_id: AgentId, tick: u64) {
        self.policing_actions.push(PolicingAction { agent_id, tick });
    }

    /// Record that an agent voted on a punishment decision.
    ///
    /// This is a signal for court system detection.
    pub fn record_punishment_vote(&mut self, voter: AgentId) {
        self.punishment_voters.insert(voter);
    }

    /// Get the crime rate (crimes per tick) over a window.
    ///
    /// Returns the average number of crimes per tick within the window.
    pub fn get_crime_rate(
        &self,
        current_tick: u64,
        window_size: u64,
    ) -> Result<Decimal, AgentError> {
        if window_size == 0 {
            return Ok(Decimal::ZERO);
        }

        let window_start = current_tick.saturating_sub(window_size);

        let crime_count: u64 = self
            .crimes
            .values()
            .filter(|c| c.tick >= window_start && c.tick <= current_tick)
            .count() as u64;

        let count_dec = Decimal::from(crime_count);
        let window_dec = Decimal::from(window_size);

        count_dec
            .checked_div(window_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("crime rate division overflow"),
            })
    }

    /// Get the detection rate (percentage of crimes that were detected).
    ///
    /// Returns a value between 0.0 and 1.0.
    pub fn get_detection_rate(&self) -> Result<Decimal, AgentError> {
        let total = self.crimes.len();
        if total == 0 {
            return Ok(Decimal::ZERO);
        }

        let detected = self.crimes.values().filter(|c| c.detected).count();

        let detected_dec = Decimal::from(detected as u64);
        let total_dec = Decimal::from(total as u64);

        detected_dec
            .checked_div(total_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("detection rate division overflow"),
            })
    }

    /// Get the punishment rate (percentage of detected crimes that were punished).
    ///
    /// Returns a value between 0.0 and 1.0.
    pub fn get_punishment_rate(&self) -> Result<Decimal, AgentError> {
        let detected: Vec<&CrimeRecord> = self
            .crimes
            .values()
            .filter(|c| c.detected)
            .collect();

        let detected_count = detected.len();
        if detected_count == 0 {
            return Ok(Decimal::ZERO);
        }

        let punished_count = detected.iter().filter(|c| c.punished).count();

        let punished_dec = Decimal::from(punished_count as u64);
        let detected_dec = Decimal::from(detected_count as u64);

        punished_dec
            .checked_div(detected_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("punishment rate division overflow"),
            })
    }

    /// Get the recidivism rate (percentage of punished agents who commit again).
    ///
    /// An agent is a recidivist if they have been punished for at least one
    /// crime and then committed another crime after that punishment.
    ///
    /// Returns a value between 0.0 and 1.0.
    pub fn get_recidivism_rate(&self) -> Result<Decimal, AgentError> {
        // Find all agents who have been punished
        let punished_agents: BTreeSet<AgentId> = self
            .crimes
            .values()
            .filter(|c| c.punished)
            .map(|c| c.perpetrator)
            .collect();

        let punished_count = punished_agents.len();
        if punished_count == 0 {
            return Ok(Decimal::ZERO);
        }

        // Find which of those agents committed a subsequent crime
        let mut recidivists: u64 = 0;

        for agent in &punished_agents {
            // Get the tick of their latest punishment
            let latest_punishment_tick = self
                .crimes
                .values()
                .filter(|c| c.perpetrator == *agent && c.punished)
                .map(|c| c.tick)
                .max()
                .unwrap_or(0);

            // Check if they committed a crime after that
            let committed_after = self
                .crimes
                .values()
                .any(|c| c.perpetrator == *agent && c.tick > latest_punishment_tick);

            if committed_after {
                recidivists = recidivists.saturating_add(1);
            }
        }

        let recidivist_dec = Decimal::from(recidivists);
        let punished_dec = Decimal::from(punished_count as u64);

        recidivist_dec
            .checked_div(punished_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("recidivism rate division overflow"),
            })
    }

    /// Classify the justice system based on observed patterns.
    ///
    /// Classification logic:
    /// - No punishments at all: `NoJustice`
    /// - Punishment voters exist (formal voting before punishment): `CourtSystem`
    /// - Victims are the primary punishers: `SelfPolicing`
    /// - A small set of agents (<=3) do most (>50%) of the punishing: `CentralizedPolicing`
    /// - Otherwise: `VigilanteJustice`
    pub fn classify_justice_system(&self) -> JusticePattern {
        let total_punishments: u64 = self
            .agent_punishment_count
            .values()
            .map(|c| u64::from(*c))
            .sum();

        if total_punishments == 0 {
            return JusticePattern::NoJustice;
        }

        // Check for court system: punishment voters exist
        if !self.punishment_voters.is_empty() {
            return JusticePattern::CourtSystem;
        }

        // Check for self-policing: victims are the primary punishers
        let victim_punishments = self.count_victim_punishments();
        let half_total = total_punishments.saturating_add(1) / 2;
        if victim_punishments > half_total {
            return JusticePattern::SelfPolicing;
        }

        // Check for centralized policing: a small set does most punishing
        let mut sorted_punishers: Vec<(AgentId, u32)> = self
            .agent_punishment_count
            .iter()
            .map(|(id, count)| (*id, *count))
            .collect();
        sorted_punishers.sort_by(|a, b| b.1.cmp(&a.1));

        // Top 3 punishers' total
        let top_3_count: u64 = sorted_punishers
            .iter()
            .take(3)
            .map(|(_, c)| u64::from(*c))
            .sum();

        if top_3_count > half_total && sorted_punishers.len() > 3 {
            return JusticePattern::CentralizedPolicing;
        }

        JusticePattern::VigilanteJustice
    }

    /// Get the most common crime types, sorted by frequency (descending).
    pub fn get_most_common_crimes(&self) -> Vec<(CrimeType, u32)> {
        let mut counts: BTreeMap<u8, (CrimeType, u32)> = BTreeMap::new();

        for crime in self.crimes.values() {
            let key = crime_type_key(crime.crime_type);
            let entry = counts.entry(key).or_insert((crime.crime_type, 0));
            entry.1 = entry.1.saturating_add(1);
        }

        let mut result: Vec<(CrimeType, u32)> = counts.into_values().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Get serial offenders (agents with 3 or more crimes).
    pub fn get_serial_offenders(&self) -> Vec<(AgentId, u32)> {
        self.agent_crime_count
            .iter()
            .filter(|(_, count)| **count >= 3)
            .map(|(id, count)| (*id, *count))
            .collect()
    }

    /// Get crime counts by location (hotspot analysis).
    ///
    /// Returns a map from location to number of crimes that occurred there.
    pub fn crime_by_location(&self) -> BTreeMap<LocationId, u32> {
        let mut counts = BTreeMap::new();

        for crime in self.crimes.values() {
            if let Some(location) = crime.location {
                let count = counts.entry(location).or_insert(0_u32);
                *count = count.saturating_add(1);
            }
        }

        counts
    }

    /// Get the total number of crimes recorded.
    pub fn total_crimes(&self) -> usize {
        self.crimes.len()
    }

    /// Get a crime record by its ID.
    pub fn get_crime(&self, crime_id: &Uuid) -> Option<&CrimeRecord> {
        self.crimes.get(crime_id)
    }

    /// Get punishments for a specific crime.
    pub fn get_punishments_for_crime(&self, crime_id: &Uuid) -> Vec<&PunishmentRecord> {
        self.punishments
            .get(crime_id)
            .map_or_else(Vec::new, |v| v.iter().collect())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Count how many punishments were administered by the victim of the crime.
    fn count_victim_punishments(&self) -> u64 {
        let mut count: u64 = 0;

        for (crime_id, punishments) in &self.punishments {
            if let Some(crime) = self.crimes.get(crime_id)
                && let Some(victim) = crime.victim
            {
                for punishment in punishments {
                    if punishment.punished_by == victim {
                        count = count.saturating_add(1);
                    }
                }
            }
        }

        count
    }
}

impl Default for CrimeTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a [`CrimeType`] to a stable integer key for aggregation.
const fn crime_type_key(crime_type: CrimeType) -> u8 {
    match crime_type {
        CrimeType::Theft => 0,
        CrimeType::Assault => 1,
        CrimeType::Murder => 2,
        CrimeType::Deception => 3,
        CrimeType::Trespass => 4,
        CrimeType::RuleViolation => 5,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_crime(
        crime_type: CrimeType,
        perpetrator: AgentId,
        victim: Option<AgentId>,
        location: Option<LocationId>,
        tick: u64,
        detected: bool,
    ) -> CrimeRecord {
        CrimeRecord {
            id: Uuid::now_v7(),
            tick,
            crime_type,
            perpetrator,
            victim,
            location,
            detected,
            punished: false,
        }
    }

    fn make_punishment(
        crime_id: Uuid,
        punished_by: AgentId,
        tick: u64,
        punishment_type: PunishmentType,
    ) -> PunishmentRecord {
        PunishmentRecord {
            crime_id,
            punished_by,
            tick,
            punishment_type,
            details: String::from("test punishment"),
        }
    }

    // -----------------------------------------------------------------------
    // Crime recording
    // -----------------------------------------------------------------------

    #[test]
    fn record_crime_increments_count() {
        let mut tracker = CrimeTracker::new();
        let perp = AgentId::new();
        let victim = AgentId::new();
        let loc = LocationId::new();

        let crime = make_crime(CrimeType::Theft, perp, Some(victim), Some(loc), 10, true);
        let _id = tracker.record_crime(crime);

        assert_eq!(tracker.total_crimes(), 1);
        assert_eq!(tracker.agent_crime_count.get(&perp), Some(&1));
    }

    #[test]
    fn record_multiple_crimes_same_agent() {
        let mut tracker = CrimeTracker::new();
        let perp = AgentId::new();

        for i in 0_u64..5 {
            let crime = make_crime(CrimeType::Theft, perp, None, None, i, true);
            let _id = tracker.record_crime(crime);
        }

        assert_eq!(tracker.total_crimes(), 5);
        assert_eq!(tracker.agent_crime_count.get(&perp), Some(&5));
    }

    // -----------------------------------------------------------------------
    // Punishment recording
    // -----------------------------------------------------------------------

    #[test]
    fn record_punishment_marks_crime() {
        let mut tracker = CrimeTracker::new();
        let perp = AgentId::new();
        let punisher = AgentId::new();

        let crime = make_crime(CrimeType::Assault, perp, None, None, 10, true);
        let crime_id = tracker.record_crime(crime);

        let punishment = make_punishment(crime_id, punisher, 15, PunishmentType::Exile);
        let result = tracker.record_punishment(punishment);
        assert!(result.is_ok());

        let crime = tracker.get_crime(&crime_id);
        assert!(crime.is_some_and(|c| c.punished));
    }

    #[test]
    fn record_punishment_nonexistent_crime_fails() {
        let mut tracker = CrimeTracker::new();
        let punisher = AgentId::new();

        let punishment = make_punishment(Uuid::nil(), punisher, 15, PunishmentType::Exile);
        let result = tracker.record_punishment(punishment);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Crime rate
    // -----------------------------------------------------------------------

    #[test]
    fn crime_rate_calculation() {
        let mut tracker = CrimeTracker::new();
        let perp = AgentId::new();

        // 10 crimes over ticks 1-10
        for i in 1_u64..=10 {
            let crime = make_crime(CrimeType::Theft, perp, None, None, i, true);
            let _id = tracker.record_crime(crime);
        }

        let rate = tracker.get_crime_rate(10, 10);
        assert!(rate.is_ok());
        // 10 crimes / 10 ticks = 1.0
        assert_eq!(rate.unwrap_or(Decimal::ZERO), Decimal::ONE);
    }

    #[test]
    fn crime_rate_zero_window() {
        let tracker = CrimeTracker::new();
        let rate = tracker.get_crime_rate(10, 0);
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Detection rate
    // -----------------------------------------------------------------------

    #[test]
    fn detection_rate_all_detected() {
        let mut tracker = CrimeTracker::new();

        for i in 0_u64..5 {
            let crime = make_crime(CrimeType::Theft, AgentId::new(), None, None, i, true);
            let _id = tracker.record_crime(crime);
        }

        let rate = tracker.get_detection_rate();
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ZERO), Decimal::ONE);
    }

    #[test]
    fn detection_rate_half_detected() {
        let mut tracker = CrimeTracker::new();

        for i in 0_u64..4 {
            let detected = i < 2;
            let crime = make_crime(CrimeType::Theft, AgentId::new(), None, None, i, detected);
            let _id = tracker.record_crime(crime);
        }

        let rate = tracker.get_detection_rate();
        assert!(rate.is_ok());
        // 2 detected / 4 total = 0.5
        assert_eq!(rate.unwrap_or(Decimal::ZERO), Decimal::new(5, 1));
    }

    #[test]
    fn detection_rate_no_crimes() {
        let tracker = CrimeTracker::new();
        let rate = tracker.get_detection_rate();
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Punishment rate
    // -----------------------------------------------------------------------

    #[test]
    fn punishment_rate_all_punished() {
        let mut tracker = CrimeTracker::new();
        let punisher = AgentId::new();

        for i in 0_u64..3 {
            let crime = make_crime(CrimeType::Assault, AgentId::new(), None, None, i, true);
            let crime_id = tracker.record_crime(crime);

            let punishment = make_punishment(crime_id, punisher, i.saturating_add(1), PunishmentType::Restitution);
            let _ = tracker.record_punishment(punishment);
        }

        let rate = tracker.get_punishment_rate();
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ZERO), Decimal::ONE);
    }

    #[test]
    fn punishment_rate_none_punished() {
        let mut tracker = CrimeTracker::new();

        for i in 0_u64..3 {
            let crime = make_crime(CrimeType::Theft, AgentId::new(), None, None, i, true);
            let _id = tracker.record_crime(crime);
        }

        let rate = tracker.get_punishment_rate();
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Recidivism rate
    // -----------------------------------------------------------------------

    #[test]
    fn recidivism_rate_repeat_offender() {
        let mut tracker = CrimeTracker::new();
        let perp = AgentId::new();
        let punisher = AgentId::new();

        // First crime at tick 1, punished at tick 2
        let crime1 = make_crime(CrimeType::Theft, perp, None, None, 1, true);
        let crime1_id = tracker.record_crime(crime1);
        let punishment = make_punishment(crime1_id, punisher, 2, PunishmentType::Restitution);
        let _ = tracker.record_punishment(punishment);

        // Second crime at tick 5 (after punishment)
        let crime2 = make_crime(CrimeType::Theft, perp, None, None, 5, true);
        let _id = tracker.record_crime(crime2);

        let rate = tracker.get_recidivism_rate();
        assert!(rate.is_ok());
        // 1 punished agent committed again / 1 total punished = 1.0
        assert_eq!(rate.unwrap_or(Decimal::ZERO), Decimal::ONE);
    }

    #[test]
    fn recidivism_rate_no_repeat() {
        let mut tracker = CrimeTracker::new();
        let punisher = AgentId::new();

        // Two different agents commit crimes, both punished, neither reoffends
        for i in 0_u64..2 {
            let perp = AgentId::new();
            let crime = make_crime(CrimeType::Theft, perp, None, None, i, true);
            let crime_id = tracker.record_crime(crime);
            let punishment = make_punishment(crime_id, punisher, i.saturating_add(1), PunishmentType::Exile);
            let _ = tracker.record_punishment(punishment);
        }

        let rate = tracker.get_recidivism_rate();
        assert!(rate.is_ok());
        assert_eq!(rate.unwrap_or(Decimal::ONE), Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Justice classification
    // -----------------------------------------------------------------------

    #[test]
    fn classify_no_justice() {
        let tracker = CrimeTracker::new();
        assert_eq!(tracker.classify_justice_system(), JusticePattern::NoJustice);
    }

    #[test]
    fn classify_self_policing() {
        let mut tracker = CrimeTracker::new();

        // Victims punish their own offenders
        for i in 0_u64..5 {
            let perp = AgentId::new();
            let victim = AgentId::new();
            let crime = make_crime(CrimeType::Theft, perp, Some(victim), None, i, true);
            let crime_id = tracker.record_crime(crime);

            // Victim punishes
            let punishment = make_punishment(crime_id, victim, i.saturating_add(1), PunishmentType::Restitution);
            let _ = tracker.record_punishment(punishment);
        }

        assert_eq!(tracker.classify_justice_system(), JusticePattern::SelfPolicing);
    }

    #[test]
    fn classify_vigilante_justice() {
        let mut tracker = CrimeTracker::new();

        // Many different agents punish (not victims, not centralized)
        for i in 0_u64..10 {
            let perp = AgentId::new();
            let victim = AgentId::new();
            let punisher = AgentId::new(); // Different agent each time
            let crime = make_crime(CrimeType::Assault, perp, Some(victim), None, i, true);
            let crime_id = tracker.record_crime(crime);

            let punishment = make_punishment(crime_id, punisher, i.saturating_add(1), PunishmentType::PhysicalPunishment);
            let _ = tracker.record_punishment(punishment);
        }

        assert_eq!(tracker.classify_justice_system(), JusticePattern::VigilanteJustice);
    }

    #[test]
    fn classify_centralized_policing() {
        let mut tracker = CrimeTracker::new();
        let sheriff = AgentId::new();

        // One agent (sheriff) does most of the punishing
        for i in 0_u64..10 {
            let perp = AgentId::new();
            let crime = make_crime(CrimeType::Theft, perp, None, None, i, true);
            let crime_id = tracker.record_crime(crime);

            let punishment = make_punishment(crime_id, sheriff, i.saturating_add(1), PunishmentType::ResourceConfiscation);
            let _ = tracker.record_punishment(punishment);
        }

        // Add a few from other agents to make the count > 3
        for i in 10_u64..14 {
            let perp = AgentId::new();
            let other_punisher = AgentId::new();
            let crime = make_crime(CrimeType::Theft, perp, None, None, i, true);
            let crime_id = tracker.record_crime(crime);

            let punishment = make_punishment(crime_id, other_punisher, i.saturating_add(1), PunishmentType::SocialShaming);
            let _ = tracker.record_punishment(punishment);
        }

        assert_eq!(tracker.classify_justice_system(), JusticePattern::CentralizedPolicing);
    }

    #[test]
    fn classify_court_system() {
        let mut tracker = CrimeTracker::new();
        let punisher = AgentId::new();

        let crime = make_crime(CrimeType::Murder, AgentId::new(), None, None, 1, true);
        let crime_id = tracker.record_crime(crime);
        let punishment = make_punishment(crime_id, punisher, 2, PunishmentType::Exile);
        let _ = tracker.record_punishment(punishment);

        // Record that a vote preceded the punishment
        tracker.record_punishment_vote(AgentId::new());

        assert_eq!(tracker.classify_justice_system(), JusticePattern::CourtSystem);
    }

    // -----------------------------------------------------------------------
    // Most common crimes
    // -----------------------------------------------------------------------

    #[test]
    fn most_common_crimes_sorted() {
        let mut tracker = CrimeTracker::new();

        // 5 thefts, 2 assaults, 1 deception
        for i in 0_u64..5 {
            let crime = make_crime(CrimeType::Theft, AgentId::new(), None, None, i, true);
            let _id = tracker.record_crime(crime);
        }
        for i in 5_u64..7 {
            let crime = make_crime(CrimeType::Assault, AgentId::new(), None, None, i, true);
            let _id = tracker.record_crime(crime);
        }
        let crime = make_crime(CrimeType::Deception, AgentId::new(), None, None, 8, true);
        let _id = tracker.record_crime(crime);

        let common = tracker.get_most_common_crimes();
        assert_eq!(common.len(), 3);
        assert!(common.first().is_some_and(|(t, c)| *t == CrimeType::Theft && *c == 5));
        assert!(common.get(1).is_some_and(|(t, c)| *t == CrimeType::Assault && *c == 2));
        assert!(common.get(2).is_some_and(|(t, c)| *t == CrimeType::Deception && *c == 1));
    }

    // -----------------------------------------------------------------------
    // Serial offenders
    // -----------------------------------------------------------------------

    #[test]
    fn serial_offenders_threshold() {
        let mut tracker = CrimeTracker::new();
        let serial = AgentId::new();
        let casual = AgentId::new();

        // serial: 4 crimes
        for i in 0_u64..4 {
            let crime = make_crime(CrimeType::Theft, serial, None, None, i, true);
            let _id = tracker.record_crime(crime);
        }
        // casual: 2 crimes (below threshold)
        for i in 4_u64..6 {
            let crime = make_crime(CrimeType::Theft, casual, None, None, i, true);
            let _id = tracker.record_crime(crime);
        }

        let offenders = tracker.get_serial_offenders();
        assert_eq!(offenders.len(), 1);
        assert!(offenders.first().is_some_and(|(id, count)| *id == serial && *count == 4));
    }

    // -----------------------------------------------------------------------
    // Crime by location
    // -----------------------------------------------------------------------

    #[test]
    fn crime_by_location_hotspots() {
        let mut tracker = CrimeTracker::new();
        let loc_a = LocationId::new();
        let loc_b = LocationId::new();

        // 5 crimes at loc_a, 2 at loc_b
        for i in 0_u64..5 {
            let crime = make_crime(CrimeType::Theft, AgentId::new(), None, Some(loc_a), i, true);
            let _id = tracker.record_crime(crime);
        }
        for i in 5_u64..7 {
            let crime = make_crime(CrimeType::Assault, AgentId::new(), None, Some(loc_b), i, true);
            let _id = tracker.record_crime(crime);
        }

        let hotspots = tracker.crime_by_location();
        assert_eq!(hotspots.get(&loc_a), Some(&5));
        assert_eq!(hotspots.get(&loc_b), Some(&2));
    }

    #[test]
    fn crime_by_location_no_location() {
        let mut tracker = CrimeTracker::new();

        let crime = make_crime(CrimeType::Deception, AgentId::new(), None, None, 1, true);
        let _id = tracker.record_crime(crime);

        let hotspots = tracker.crime_by_location();
        assert!(hotspots.is_empty());
    }
}
