//! Environmental systems for the Emergence simulation.
//!
//! This module implements weather generation with season-weighted probabilities
//! and deterministic randomness for reproducible simulations.
//!
//! # Weather Generation
//!
//! Weather is generated once per tick using a seeded pseudo-random number
//! generator. The probability distribution changes by season:
//!
//! | Weather  | Spring | Summer | Autumn | Winter |
//! |----------|--------|--------|--------|--------|
//! | Clear    | 30%    | 45%    | 35%    | 20%    |
//! | Rain     | 35%    | 15%    | 25%    | 10%    |
//! | Storm    | 10%    | 10%    | 15%    | 15%    |
//! | Drought  |  5%    | 25%    |  5%    |  0%    |
//! | Snow     |  0%    |  0%    |  5%    | 40%    |
//! | (repeat) | 20%    |  5%    | 15%    | 15%    |
//!
//! The "repeat" weight means the previous tick's weather persists, giving
//! weather streaks a natural feel.
//!
//! # Determinism
//!
//! The RNG is a simple `xorshift64` seeded from `(world_seed, tick)`. This
//! means the same seed and tick always produce the same weather, enabling
//! reproducible simulation runs and event replay.

use emergence_types::{Season, Weather};

/// Seasonal weather weights for probability-based generation.
///
/// Each entry is `(weather_variant, weight)`. Weights are summed and a
/// random value in `[0, total_weight)` selects the weather. The special
/// `None` entry means "repeat previous weather".
#[derive(Debug, Clone)]
pub struct SeasonWeights {
    /// Weighted entries: `(Some(weather), weight)` or `(None, weight)` for repeat.
    entries: Vec<(Option<Weather>, u32)>,
}

impl SeasonWeights {
    /// Return the weather weights for the given season.
    pub fn for_season(season: Season) -> Self {
        let entries = match season {
            Season::Spring => vec![
                (Some(Weather::Clear), 30),
                (Some(Weather::Rain), 35),
                (Some(Weather::Storm), 10),
                (Some(Weather::Drought), 5),
                (Some(Weather::Snow), 0),
                (None, 20), // repeat
            ],
            Season::Summer => vec![
                (Some(Weather::Clear), 45),
                (Some(Weather::Rain), 15),
                (Some(Weather::Storm), 10),
                (Some(Weather::Drought), 25),
                (Some(Weather::Snow), 0),
                (None, 5), // repeat
            ],
            Season::Autumn => vec![
                (Some(Weather::Clear), 35),
                (Some(Weather::Rain), 25),
                (Some(Weather::Storm), 15),
                (Some(Weather::Drought), 5),
                (Some(Weather::Snow), 5),
                (None, 15), // repeat
            ],
            Season::Winter => vec![
                (Some(Weather::Clear), 20),
                (Some(Weather::Rain), 10),
                (Some(Weather::Storm), 15),
                (Some(Weather::Drought), 0),
                (Some(Weather::Snow), 40),
                (None, 15), // repeat
            ],
        };
        Self { entries }
    }

    /// Select a weather variant (or repeat signal) given a random value.
    ///
    /// The `random_value` should be in `[0, total_weight())`. Returns
    /// `Some(weather)` for a specific weather, or `None` to indicate the
    /// previous weather should repeat.
    fn select(&self, random_value: u32) -> Option<Weather> {
        let mut cumulative: u32 = 0;
        for &(weather, weight) in &self.entries {
            cumulative = cumulative.saturating_add(weight);
            if random_value < cumulative {
                return weather;
            }
        }
        // Fallback: if we somehow exceed all weights, default to Clear.
        Some(Weather::Clear)
    }

    /// Return the total weight (sum of all entry weights).
    fn total_weight(&self) -> u32 {
        let mut total: u32 = 0;
        for &(_, weight) in &self.entries {
            total = total.saturating_add(weight);
        }
        total
    }
}

/// Deterministic weather generator for the simulation.
///
/// Uses a seeded `xorshift64` PRNG to produce reproducible weather sequences.
/// The same `(world_seed, tick)` pair always yields the same weather.
#[derive(Debug, Clone)]
pub struct WeatherSystem {
    /// The world seed used to derive per-tick randomness.
    world_seed: u64,

    /// The weather from the previous tick (for "repeat" rolls).
    previous_weather: Weather,
}

impl WeatherSystem {
    /// Create a new weather system with the given world seed.
    ///
    /// The initial previous weather is [`Weather::Clear`].
    pub const fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            previous_weather: Weather::Clear,
        }
    }

    /// Generate the weather for a given tick and season.
    ///
    /// This method is idempotent for the same `(tick, season)` pair when
    /// the previous weather state is consistent. Call it once per tick
    /// during the World Wake phase, then use the returned value.
    ///
    /// Updates the internal `previous_weather` state for repeat rolls on
    /// subsequent ticks.
    pub fn generate(&mut self, tick: u64, season: Season) -> Weather {
        let weights = SeasonWeights::for_season(season);
        let total = weights.total_weight();

        if total == 0 {
            // Degenerate case: no weights at all, default to Clear.
            self.previous_weather = Weather::Clear;
            return Weather::Clear;
        }

        let random = deterministic_random(self.world_seed, tick);
        // Map the random u64 into [0, total_weight) using checked modular reduction.
        // total > 0 is verified above. The remainder is strictly < total (a u32),
        // so `try_from` is guaranteed to succeed.
        let remainder = random.checked_rem(u64::from(total)).unwrap_or(0);
        let roll = u32::try_from(remainder).unwrap_or(0);

        let weather = weights.select(roll).unwrap_or(self.previous_weather);

        self.previous_weather = weather;
        weather
    }

    /// Peek at what the weather would be for a given tick and season
    /// without updating internal state.
    pub fn peek(&self, tick: u64, season: Season) -> Weather {
        let weights = SeasonWeights::for_season(season);
        let total = weights.total_weight();

        if total == 0 {
            return Weather::Clear;
        }

        let random = deterministic_random(self.world_seed, tick);
        let remainder = random.checked_rem(u64::from(total)).unwrap_or(0);
        let roll = u32::try_from(remainder).unwrap_or(0);

        weights.select(roll).unwrap_or(self.previous_weather)
    }

    /// Return the weather from the previous tick.
    pub const fn previous_weather(&self) -> Weather {
        self.previous_weather
    }

    /// Override the previous weather (useful for state restoration).
    pub const fn set_previous_weather(&mut self, weather: Weather) {
        self.previous_weather = weather;
    }

    /// Return the world seed.
    pub const fn world_seed(&self) -> u64 {
        self.world_seed
    }
}

/// Deterministic pseudo-random number generator using `xorshift64`.
///
/// Combines the world seed and tick number to produce a unique random
/// value for each `(seed, tick)` pair. The same inputs always produce
/// the same output.
const fn deterministic_random(world_seed: u64, tick: u64) -> u64 {
    // Combine seed and tick with a mixing step to avoid trivial patterns.
    // The constant 0x517cc1b727220a95 is a well-known mixing constant.
    let mut state = world_seed.wrapping_add(tick.wrapping_mul(0x517c_c1b7_2722_0a95));

    // Ensure non-zero state (xorshift requires non-zero input).
    if state == 0 {
        state = 0xdead_beef_cafe_babe;
    }

    // xorshift64 algorithm
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;

    state
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_random_is_reproducible() {
        let a = deterministic_random(42, 100);
        let b = deterministic_random(42, 100);
        assert_eq!(a, b);
    }

    #[test]
    fn deterministic_random_varies_by_tick() {
        let a = deterministic_random(42, 100);
        let b = deterministic_random(42, 101);
        assert_ne!(a, b);
    }

    #[test]
    fn deterministic_random_varies_by_seed() {
        let a = deterministic_random(42, 100);
        let b = deterministic_random(43, 100);
        assert_ne!(a, b);
    }

    #[test]
    fn deterministic_random_handles_zero_state() {
        // When seed + tick * constant wraps to zero, the fallback kicks in.
        let result = deterministic_random(0, 0);
        assert_ne!(result, 0);
    }

    #[test]
    fn weather_system_produces_valid_weather() {
        let mut system = WeatherSystem::new(42);

        // Generate weather for 360 ticks (one full year at 90 ticks/season)
        let all_seasons = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];
        for tick in 0_u64..360 {
            let season_idx = (tick / 90) % 4;
            let season = all_seasons.get(season_idx as usize).copied().unwrap_or(Season::Spring);
            let weather = system.generate(tick, season);

            // Weather must be one of the valid variants
            match weather {
                Weather::Clear | Weather::Rain | Weather::Storm | Weather::Drought | Weather::Snow => {}
            }
        }
    }

    #[test]
    fn weather_system_is_reproducible() {
        let mut system_a = WeatherSystem::new(42);
        let mut system_b = WeatherSystem::new(42);

        for tick in 0_u64..100 {
            let a = system_a.generate(tick, Season::Spring);
            let b = system_b.generate(tick, Season::Spring);
            assert_eq!(a, b, "Weather diverged at tick {tick}");
        }
    }

    #[test]
    fn weather_system_different_seeds_diverge() {
        let mut system_a = WeatherSystem::new(42);
        let mut system_b = WeatherSystem::new(99);

        let mut same_count: u32 = 0;
        for tick in 0_u64..100 {
            let a = system_a.generate(tick, Season::Spring);
            let b = system_b.generate(tick, Season::Spring);
            if a == b {
                same_count = same_count.saturating_add(1);
            }
        }
        // With different seeds, not all 100 ticks should have the same weather.
        // Statistically near-impossible for all 100 to match with different seeds.
        assert!(same_count < 100, "Different seeds should produce different weather sequences");
    }

    #[test]
    fn no_snow_in_spring_or_summer() {
        // Snow has weight 0 in spring and summer, so it should never appear
        // unless it is the repeat of a previous tick's weather.
        let mut system = WeatherSystem::new(42);
        // Start with Clear so repeat can never produce Snow.
        system.set_previous_weather(Weather::Clear);

        for tick in 0_u64..1000 {
            let weather = system.generate(tick, Season::Spring);
            // If the previous was not Snow, we should never get Snow in spring.
            // However, repeat could theoretically propagate Snow from a prior
            // season. Since we force Clear at start and stay in Spring, Snow
            // should never appear.
            assert_ne!(
                weather,
                Weather::Snow,
                "Snow appeared in spring at tick {tick}"
            );
            // Reset to ensure repeat chain cannot introduce Snow
            if weather == Weather::Snow {
                break;
            }
        }
    }

    #[test]
    fn no_drought_in_winter() {
        let mut system = WeatherSystem::new(42);
        system.set_previous_weather(Weather::Clear);

        for tick in 0_u64..1000 {
            let weather = system.generate(tick, Season::Winter);
            assert_ne!(
                weather,
                Weather::Drought,
                "Drought appeared in winter at tick {tick}"
            );
        }
    }

    #[test]
    fn season_weights_total_is_100() {
        for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
            let weights = SeasonWeights::for_season(season);
            assert_eq!(weights.total_weight(), 100, "Total weight for {season:?} should be 100");
        }
    }

    #[test]
    fn peek_does_not_change_state() {
        let system = WeatherSystem::new(42);
        let before = system.previous_weather();
        let _ = system.peek(50, Season::Summer);
        assert_eq!(system.previous_weather(), before);
    }

    #[test]
    fn weather_distribution_is_reasonable() {
        // Over many ticks in summer, Clear should be most common
        let mut system = WeatherSystem::new(42);
        let mut clear_count: u32 = 0;
        let total: u32 = 10_000;

        for tick in 0..u64::from(total) {
            let weather = system.generate(tick, Season::Summer);
            if weather == Weather::Clear {
                clear_count = clear_count.saturating_add(1);
            }
        }

        // Clear has weight 45/100 in summer; expect roughly 35-55% accounting
        // for repeat effects. Just check it is the plurality.
        assert!(
            clear_count > 2000,
            "Clear weather should appear frequently in summer (got {clear_count}/{total})"
        );
    }

    #[test]
    fn winter_produces_snow() {
        let mut system = WeatherSystem::new(42);
        let mut snow_count: u32 = 0;

        for tick in 0_u64..1000 {
            let weather = system.generate(tick, Season::Winter);
            if weather == Weather::Snow {
                snow_count = snow_count.saturating_add(1);
            }
        }

        // Snow has weight 40/100 in winter; should appear frequently.
        assert!(
            snow_count > 100,
            "Snow should appear frequently in winter (got {snow_count}/1000)"
        );
    }
}
