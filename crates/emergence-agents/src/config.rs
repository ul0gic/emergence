//! Configuration constants and defaults for agent vital mechanics.
//!
//! These values correspond to the parameters defined in
//! `world-engine.md` section 6.2 and `emergence-config.yaml` under the
//! `economy` and `population` keys. The [`VitalsConfig`] struct bundles
//! every tunable so that callers (tick cycle, tests) can override defaults.

/// Configuration for agent vital mechanics applied each tick.
///
/// All rates are expressed as whole `u32` values applied once per tick.
/// The World Engine constructs this from `emergence-config.yaml` at
/// simulation start and passes it into vital update functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VitalsConfig {
    /// Hunger points added per tick (default: 5).
    pub hunger_rate: u32,

    /// Health damage per tick when hunger >= 100 (default: 10).
    pub starvation_damage: u32,

    /// Energy recovered when resting without shelter (default: 30).
    pub rest_recovery: u32,

    /// Health recovered per tick when conditions are met (default: 2).
    ///
    /// Conditions: hunger < 50, energy > 50, sheltered.
    pub natural_heal_rate: u32,

    /// Maximum age in ticks before death by old age (default: 2500).
    pub lifespan: u32,

    /// Default carry capacity for new agents (default: 50).
    pub carry_capacity: u32,

    /// Starting energy for new agents (default: 80).
    pub starting_energy: u32,

    /// Starting health for new agents (default: 100).
    pub starting_health: u32,

    /// Hunger threshold above which starvation damage applies (default: 100).
    pub starvation_threshold: u32,

    /// Hunger threshold below which health can regenerate (default: 50).
    pub heal_hunger_threshold: u32,

    /// Energy threshold above which health can regenerate (default: 50).
    pub heal_energy_threshold: u32,

    /// Fraction of lifespan at which energy cap begins declining (default: 80%).
    ///
    /// Stored as a percentage (0--100). At `aging_threshold_pct` percent of
    /// lifespan, the agent's maximum energy starts decreasing.
    pub aging_threshold_pct: u32,
}

impl Default for VitalsConfig {
    fn default() -> Self {
        Self {
            hunger_rate: 5,
            starvation_damage: 10,
            rest_recovery: 30,
            natural_heal_rate: 2,
            lifespan: 2500,
            carry_capacity: 50,
            starting_energy: 80,
            starting_health: 100,
            starvation_threshold: 100,
            heal_hunger_threshold: 50,
            heal_energy_threshold: 50,
            aging_threshold_pct: 80,
        }
    }
}

impl VitalsConfig {
    /// Compute the maximum energy an agent can have at the given age.
    ///
    /// Before `aging_threshold_pct` of lifespan, max energy is 100.
    /// After that threshold, max energy linearly decreases to 50 at death.
    ///
    /// Formula from `world-engine.md` section 6.2:
    /// ```text
    /// max_energy = 100 * (1 - ((age - lifespan * 0.8) / (lifespan * 0.2)) * 0.5)
    /// ```
    ///
    /// Returns `None` if an arithmetic overflow occurs.
    pub fn max_energy_for_age(&self, age: u32) -> Option<u32> {
        // Compute the aging threshold in ticks
        let threshold = self.lifespan.checked_mul(self.aging_threshold_pct)?.checked_div(100)?;

        if age <= threshold {
            return Some(100);
        }

        // Age beyond the threshold
        let age_beyond = age.checked_sub(threshold)?;
        // The old-age window is lifespan - threshold
        let old_age_window = self.lifespan.checked_sub(threshold)?;

        if old_age_window == 0 {
            // Edge case: threshold equals lifespan, no decline period
            return Some(100);
        }

        // Decline ratio: age_beyond / old_age_window, scaled by 50 (the max decline).
        // max_energy = 100 - (age_beyond * 50 / old_age_window)
        // We compute (age_beyond * 50) first, using checked arithmetic.
        let decline_numerator = age_beyond.checked_mul(50)?;
        let decline = decline_numerator.checked_div(old_age_window)?;

        // Clamp decline to at most 50 (energy floor is 50).
        let clamped_decline = if decline > 50 { 50 } else { decline };

        100_u32.checked_sub(clamped_decline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = VitalsConfig::default();
        assert_eq!(cfg.hunger_rate, 5);
        assert_eq!(cfg.starvation_damage, 10);
        assert_eq!(cfg.rest_recovery, 30);
        assert_eq!(cfg.natural_heal_rate, 2);
        assert_eq!(cfg.lifespan, 2500);
        assert_eq!(cfg.carry_capacity, 50);
        assert_eq!(cfg.starting_energy, 80);
        assert_eq!(cfg.starting_health, 100);
    }

    #[test]
    fn max_energy_before_threshold() {
        let cfg = VitalsConfig::default();
        // At age 0, 1000, 2000 -- all below 80% of 2500 (=2000)
        assert_eq!(cfg.max_energy_for_age(0), Some(100));
        assert_eq!(cfg.max_energy_for_age(1000), Some(100));
        assert_eq!(cfg.max_energy_for_age(2000), Some(100));
    }

    #[test]
    fn max_energy_at_threshold_boundary() {
        let cfg = VitalsConfig::default();
        // Exactly at threshold (2000): still 100
        assert_eq!(cfg.max_energy_for_age(2000), Some(100));
        // One tick past: very small decline
        // age_beyond = 1, old_age_window = 500, decline = 1*50/500 = 0
        assert_eq!(cfg.max_energy_for_age(2001), Some(100));
    }

    #[test]
    fn max_energy_at_lifespan() {
        let cfg = VitalsConfig::default();
        // At lifespan (2500): age_beyond=500, decline=500*50/500=50, max=50
        assert_eq!(cfg.max_energy_for_age(2500), Some(50));
    }

    #[test]
    fn max_energy_halfway_through_decline() {
        let cfg = VitalsConfig::default();
        // At age 2250: halfway through decline period
        // age_beyond=250, decline=250*50/500=25, max=75
        assert_eq!(cfg.max_energy_for_age(2250), Some(75));
    }

    #[test]
    fn max_energy_beyond_lifespan() {
        let cfg = VitalsConfig::default();
        // Beyond lifespan, decline is clamped at 50
        assert_eq!(cfg.max_energy_for_age(3000), Some(50));
    }
}
