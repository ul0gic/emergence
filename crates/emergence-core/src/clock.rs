//! World clock and time tracking for the Emergence simulation.
//!
//! The clock is the single source of truth for all temporal state in the
//! simulation. It tracks the current tick, derives the season from tick
//! count and configuration, maps ticks to time-of-day, and holds the
//! current civilizational era.
//!
//! # Design Principles
//!
//! - All temporal derivations use checked arithmetic (no silent overflow).
//! - Season and time-of-day are computed from the tick counter -- never
//!   stored independently. The tick number is the source of truth.
//! - Era transitions are set externally by the engine when emergent
//!   conditions are met.

use emergence_types::{Era, Season, TimeOfDay};

use crate::config::TimeConfig;

/// Number of time-of-day phases within a single tick-day.
const TIME_OF_DAY_PHASES: u64 = 5;

/// Errors that can occur during clock operations.
#[derive(Debug, thiserror::Error)]
pub enum ClockError {
    /// Tick counter would overflow.
    #[error("tick counter overflow: cannot advance beyond u64::MAX")]
    TickOverflow,

    /// Invalid time configuration (e.g. zero ticks per season).
    #[error("invalid time configuration: {reason}")]
    InvalidConfig {
        /// Explanation of what is wrong with the configuration.
        reason: String,
    },
}

/// World clock tracking the simulation's temporal state.
///
/// The clock advances once per tick. Season and time-of-day are derived
/// from the tick counter and the [`TimeConfig`]. The era is set externally
/// when the engine detects emergent era transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldClock {
    /// Current tick number (0-indexed, incremented at the start of each tick).
    tick: u64,

    /// Current civilizational era, set by the engine when conditions are met.
    era: Era,

    /// Number of ticks per season (from configuration).
    ticks_per_season: u64,

    /// Ordered list of seasons that form the annual cycle.
    seasons: Vec<Season>,
}

impl WorldClock {
    /// Create a new world clock from a time configuration.
    ///
    /// The clock starts at tick 0 in the [`Era::Primitive`] era. The season
    /// list must contain at least one entry, and `ticks_per_season` must be
    /// at least 1.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::InvalidConfig`] if the configuration is invalid.
    pub fn new(config: &TimeConfig) -> Result<Self, ClockError> {
        if config.ticks_per_season == 0 {
            return Err(ClockError::InvalidConfig {
                reason: "ticks_per_season must be at least 1".to_owned(),
            });
        }

        let seasons = parse_seasons(&config.seasons)?;

        if seasons.is_empty() {
            return Err(ClockError::InvalidConfig {
                reason: "at least one season must be configured".to_owned(),
            });
        }

        Ok(Self {
            tick: 0,
            era: Era::Primitive,
            ticks_per_season: config.ticks_per_season,
            seasons,
        })
    }

    /// Create a clock from explicit parameters (useful for testing and
    /// state restoration).
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::InvalidConfig`] if `ticks_per_season` is 0
    /// or the season list is empty.
    pub fn from_parts(
        tick: u64,
        era: Era,
        ticks_per_season: u64,
        seasons: Vec<Season>,
    ) -> Result<Self, ClockError> {
        if ticks_per_season == 0 {
            return Err(ClockError::InvalidConfig {
                reason: "ticks_per_season must be at least 1".to_owned(),
            });
        }
        if seasons.is_empty() {
            return Err(ClockError::InvalidConfig {
                reason: "at least one season must be configured".to_owned(),
            });
        }
        Ok(Self {
            tick,
            era,
            ticks_per_season,
            seasons,
        })
    }

    /// Advance the clock by one tick. Returns the new tick number.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::TickOverflow`] if the tick counter would exceed
    /// `u64::MAX`.
    pub fn advance(&mut self) -> Result<u64, ClockError> {
        self.tick = self.tick.checked_add(1).ok_or(ClockError::TickOverflow)?;
        Ok(self.tick)
    }

    /// Return the current tick number.
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Return the current civilizational era.
    pub const fn era(&self) -> Era {
        self.era
    }

    /// Set the era to a new value (called by the engine on era transitions).
    pub const fn set_era(&mut self, era: Era) {
        self.era = era;
    }

    /// Return the configured number of ticks per season.
    pub const fn ticks_per_season(&self) -> u64 {
        self.ticks_per_season
    }

    /// Return the number of ticks in one full year (all seasons combined).
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::InvalidConfig`] if the multiplication overflows.
    pub fn ticks_per_year(&self) -> Result<u64, ClockError> {
        let season_count = u64::try_from(self.seasons.len()).map_err(|_err| {
            ClockError::InvalidConfig {
                reason: "season count exceeds u64 range".to_owned(),
            }
        })?;
        self.ticks_per_season
            .checked_mul(season_count)
            .ok_or_else(|| ClockError::InvalidConfig {
                reason: "ticks_per_year overflow".to_owned(),
            })
    }

    /// Compute the current season from the tick counter.
    ///
    /// The season index is `(tick / ticks_per_season) % season_count`.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::InvalidConfig`] if the season list is empty
    /// or the computed index is out of bounds.
    pub fn season(&self) -> Result<Season, ClockError> {
        let season_count = u64::try_from(self.seasons.len()).map_err(|_err| {
            ClockError::InvalidConfig {
                reason: "season count exceeds u64 range".to_owned(),
            }
        })?;

        // Division is safe: ticks_per_season >= 1 and season_count >= 1
        // are guaranteed by the constructor.
        let raw_index = self.tick.checked_div(self.ticks_per_season).ok_or_else(|| {
            ClockError::InvalidConfig {
                reason: "ticks_per_season is zero".to_owned(),
            }
        })?;
        let season_index = raw_index.checked_rem(season_count).ok_or_else(|| {
            ClockError::InvalidConfig {
                reason: "season count is zero".to_owned(),
            }
        })?;

        let idx = usize::try_from(season_index).map_err(|_err| ClockError::InvalidConfig {
            reason: "season index exceeds usize range".to_owned(),
        })?;

        self.seasons
            .get(idx)
            .copied()
            .ok_or_else(|| ClockError::InvalidConfig {
                reason: format!(
                    "season index {idx} out of bounds (len {})",
                    self.seasons.len()
                ),
            })
    }

    /// Compute the index of the current season within the season list.
    ///
    /// Returns a zero-based index (0 = first configured season).
    /// Uses saturating arithmetic; returns 0 if the season list is empty.
    pub fn season_index(&self) -> u64 {
        let season_count = self.seasons.len().max(1);
        // Safe: ticks_per_season >= 1 and season_count >= 1 by construction.
        let season_count_u64 = season_count as u64;
        let raw = self.tick.checked_div(self.ticks_per_season).unwrap_or(0);
        raw.checked_rem(season_count_u64).unwrap_or(0)
    }

    /// Compute the tick offset within the current season (0-based).
    ///
    /// Uses checked arithmetic; returns 0 if `ticks_per_season` is somehow zero.
    pub fn tick_within_season(&self) -> u64 {
        self.tick
            .checked_rem(self.ticks_per_season)
            .unwrap_or(0)
    }

    /// Compute the current time of day from the tick counter.
    ///
    /// Time of day cycles through 5 phases: Dawn, Morning, Afternoon, Dusk,
    /// Night. The mapping is `tick % 5`:
    ///   0 => Dawn, 1 => Morning, 2 => Afternoon, 3 => Dusk, 4 => Night.
    pub fn time_of_day(&self) -> TimeOfDay {
        let phase = self.tick.checked_rem(TIME_OF_DAY_PHASES).unwrap_or(0);
        match phase {
            0 => TimeOfDay::Dawn,
            1 => TimeOfDay::Morning,
            2 => TimeOfDay::Afternoon,
            3 => TimeOfDay::Dusk,
            // 4 is the only remaining case (0..5 with 0-3 handled above).
            _ => TimeOfDay::Night,
        }
    }

    /// Return the number of ticks until the next season transition.
    pub fn ticks_until_season_change(&self) -> u64 {
        let within = self.tick_within_season();
        self.ticks_per_season.saturating_sub(within)
    }

    /// Return the complete season list.
    pub fn seasons(&self) -> &[Season] {
        &self.seasons
    }
}

/// Parse a list of season name strings into typed [`Season`] values.
///
/// # Errors
///
/// Returns [`ClockError::InvalidConfig`] if any string does not match a
/// known season variant.
fn parse_seasons(names: &[String]) -> Result<Vec<Season>, ClockError> {
    names
        .iter()
        .map(|name| match name.to_lowercase().as_str() {
            "spring" => Ok(Season::Spring),
            "summer" => Ok(Season::Summer),
            "autumn" | "fall" => Ok(Season::Autumn),
            "winter" => Ok(Season::Winter),
            other => Err(ClockError::InvalidConfig {
                reason: format!("unknown season: {other}"),
            }),
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Helper to create a default time config for tests.
    fn default_time_config() -> TimeConfig {
        TimeConfig {
            ticks_per_season: 90,
            seasons: vec![
                "spring".to_owned(),
                "summer".to_owned(),
                "autumn".to_owned(),
                "winter".to_owned(),
            ],
            day_night: true,
        }
    }

    /// Helper to create a clock from a `TimeConfig`, panicking in tests on failure.
    fn make_clock(cfg: &TimeConfig) -> WorldClock {
        WorldClock::new(cfg).unwrap()
    }

    #[test]
    fn clock_starts_at_tick_zero() {
        let cfg = default_time_config();
        let clock = make_clock(&cfg);
        assert_eq!(clock.tick(), 0);
        assert_eq!(clock.era(), Era::Primitive);
    }

    #[test]
    fn clock_advances() {
        let cfg = default_time_config();
        let mut clock = make_clock(&cfg);
        let result = clock.advance();
        assert!(result.is_ok());
        assert_eq!(clock.tick(), 1);

        let result = clock.advance();
        assert!(result.is_ok());
        assert_eq!(clock.tick(), 2);
    }

    #[test]
    fn season_rotates_correctly() {
        let cfg = default_time_config();
        let mut clock = make_clock(&cfg);

        // Tick 0: Spring
        assert_eq!(clock.season().unwrap(), Season::Spring);

        // Advance to tick 90: Summer
        for _ in 0..90 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick(), 90);
        assert_eq!(clock.season().unwrap(), Season::Summer);

        // Advance to tick 180: Autumn
        for _ in 0..90 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick(), 180);
        assert_eq!(clock.season().unwrap(), Season::Autumn);

        // Advance to tick 270: Winter
        for _ in 0..90 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick(), 270);
        assert_eq!(clock.season().unwrap(), Season::Winter);

        // Advance to tick 360: Spring again (year wraps)
        for _ in 0..90 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick(), 360);
        assert_eq!(clock.season().unwrap(), Season::Spring);
    }

    #[test]
    fn time_of_day_cycles() {
        let cfg = default_time_config();
        let mut clock = make_clock(&cfg);

        assert_eq!(clock.time_of_day(), TimeOfDay::Dawn); // tick 0
        let _ = clock.advance(); // tick 1
        assert_eq!(clock.time_of_day(), TimeOfDay::Morning);
        let _ = clock.advance(); // tick 2
        assert_eq!(clock.time_of_day(), TimeOfDay::Afternoon);
        let _ = clock.advance(); // tick 3
        assert_eq!(clock.time_of_day(), TimeOfDay::Dusk);
        let _ = clock.advance(); // tick 4
        assert_eq!(clock.time_of_day(), TimeOfDay::Night);
        let _ = clock.advance(); // tick 5
        assert_eq!(clock.time_of_day(), TimeOfDay::Dawn); // wraps
    }

    #[test]
    fn era_can_be_set() {
        let cfg = default_time_config();
        let mut clock = make_clock(&cfg);

        assert_eq!(clock.era(), Era::Primitive);
        clock.set_era(Era::Tribal);
        assert_eq!(clock.era(), Era::Tribal);
        clock.set_era(Era::Agricultural);
        assert_eq!(clock.era(), Era::Agricultural);
    }

    #[test]
    fn ticks_per_year_computes_correctly() {
        let cfg = default_time_config();
        let clock = make_clock(&cfg);
        // 90 ticks/season * 4 seasons = 360 ticks/year
        assert_eq!(clock.ticks_per_year().unwrap(), 360);
    }

    #[test]
    fn ticks_until_season_change() {
        let cfg = default_time_config();
        let mut clock = make_clock(&cfg);

        // At tick 0, ticks until season change = 90
        assert_eq!(clock.ticks_until_season_change(), 90);

        // Advance 45 ticks
        for _ in 0..45 {
            let _ = clock.advance();
        }
        assert_eq!(clock.ticks_until_season_change(), 45);

        // Advance to tick 89
        for _ in 0..44 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick(), 89);
        assert_eq!(clock.ticks_until_season_change(), 1);

        // Advance to tick 90 (new season)
        let _ = clock.advance();
        assert_eq!(clock.tick(), 90);
        assert_eq!(clock.ticks_until_season_change(), 90);
    }

    #[test]
    fn invalid_config_zero_ticks_per_season() {
        let cfg = TimeConfig {
            ticks_per_season: 0,
            seasons: vec!["spring".to_owned()],
            day_night: true,
        };
        let result = WorldClock::new(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_config_empty_seasons() {
        let cfg = TimeConfig {
            ticks_per_season: 90,
            seasons: vec![],
            day_night: true,
        };
        let result = WorldClock::new(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_config_unknown_season() {
        let cfg = TimeConfig {
            ticks_per_season: 90,
            seasons: vec!["monsoon".to_owned()],
            day_night: true,
        };
        let result = WorldClock::new(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn single_season_configuration() {
        let cfg = TimeConfig {
            ticks_per_season: 10,
            seasons: vec!["winter".to_owned()],
            day_night: true,
        };
        let mut clock = make_clock(&cfg);

        assert_eq!(clock.season().unwrap(), Season::Winter);
        for _ in 0..25 {
            let _ = clock.advance();
        }
        // Still winter, because there is only one season
        assert_eq!(clock.season().unwrap(), Season::Winter);
    }

    #[test]
    fn tick_within_season_wraps() {
        let cfg = TimeConfig {
            ticks_per_season: 10,
            seasons: vec!["spring".to_owned(), "summer".to_owned()],
            day_night: true,
        };
        let mut clock = make_clock(&cfg);

        assert_eq!(clock.tick_within_season(), 0);
        for _ in 0..5 {
            let _ = clock.advance();
        }
        assert_eq!(clock.tick_within_season(), 5);
        for _ in 0..5 {
            let _ = clock.advance();
        }
        // tick 10: new season, offset back to 0
        assert_eq!(clock.tick_within_season(), 0);
    }

    #[test]
    fn from_parts_restores_state() {
        let clock = WorldClock::from_parts(
            500,
            Era::Bronze,
            90,
            vec![Season::Spring, Season::Summer, Season::Autumn, Season::Winter],
        )
        .unwrap();
        assert_eq!(clock.tick(), 500);
        assert_eq!(clock.era(), Era::Bronze);
        // tick 500: 500 / 90 = 5 (integer), 5 % 4 = 1 => Summer
        assert_eq!(clock.season().unwrap(), Season::Summer);
    }

    #[test]
    fn fall_alias_for_autumn() {
        let cfg = TimeConfig {
            ticks_per_season: 10,
            seasons: vec!["fall".to_owned()],
            day_night: true,
        };
        let clock = make_clock(&cfg);
        assert_eq!(clock.season().unwrap(), Season::Autumn);
    }
}
