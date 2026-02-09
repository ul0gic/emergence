//! Skill system: proficiency levels, XP tracking, and skill effects.
//!
//! Implements `agent-system.md` section 3.6. Agents have skills that improve
//! with use. Each successful action awards XP in the relevant skill, and
//! accumulating enough XP triggers a level-up.
//!
//! # Skill Names
//!
//! The canonical skill names are: `"gathering"`, `"building"`, `"trading"`,
//! `"teaching"`, `"farming"`, `"crafting"`, `"mining"`, `"smelting"`,
//! `"combat"`, `"exploration"`.
//!
//! # Level-Up Formula
//!
//! XP required to advance from level N to level N+1 is `N * 100`.
//! For example, level 1 to 2 requires 100 XP; level 2 to 3 requires 200 XP.
//!
//! # Skill Effects
//!
//! Skill levels modify action outcomes:
//! - Gathering yield = `base_yield + (skill_level * 0.5)`
//! - Building speed = `base_time / (1 + skill_level * 0.1)`
//! - Teaching success = `base_rate + (skill_level * 0.05)`, capped at 0.99
//!
//! These are computed in the [`effects`] submodule using integer arithmetic
//! where possible, or [`rust_decimal::Decimal`] for precise fixed-point math.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum skill level an agent can reach.
pub const MAX_SKILL_LEVEL: u32 = 20;

/// All canonical skill names.
pub const SKILL_NAMES: &[&str] = &[
    "gathering",
    "building",
    "trading",
    "teaching",
    "farming",
    "crafting",
    "mining",
    "smelting",
    "combat",
    "exploration",
];

// ---------------------------------------------------------------------------
// XP Award Constants (Task 3.5.3)
// ---------------------------------------------------------------------------

/// XP awarded on a successful gather action.
pub const XP_GATHER: u32 = 10;

/// XP awarded on a successful build action.
pub const XP_BUILD: u32 = 15;

/// XP awarded on a completed trade.
pub const XP_TRADE: u32 = 5;

/// XP awarded on a successful teach action.
pub const XP_TEACH: u32 = 10;

/// XP awarded on completing a move (arriving at destination).
pub const XP_MOVE: u32 = 5;

/// XP awarded on a successful farm plant action.
pub const XP_FARM_PLANT: u32 = 10;

/// XP awarded on a successful farm harvest action.
pub const XP_FARM_HARVEST: u32 = 10;

/// XP awarded on a successful craft action.
pub const XP_CRAFT: u32 = 10;

/// XP awarded on a successful mine action.
pub const XP_MINE: u32 = 10;

/// XP awarded on a successful smelt action.
pub const XP_SMELT: u32 = 10;

// ---------------------------------------------------------------------------
// SkillSystem
// ---------------------------------------------------------------------------

/// Per-agent skill tracking with XP accumulation and level-up mechanics.
///
/// Wraps two maps (`skill_levels` and `skill_xp`) and provides methods for
/// adding XP, querying levels, and computing level-up thresholds. Skill levels
/// start at 1 when the agent first gains XP in that skill, and cap at
/// [`MAX_SKILL_LEVEL`] (20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSystem {
    /// Skill name to current level (starting at 1 once the skill is learned).
    skill_levels: BTreeMap<String, u32>,
    /// Skill name to accumulated experience points toward next level.
    skill_xp: BTreeMap<String, u32>,
}

impl SkillSystem {
    /// Create an empty skill system with no skills.
    pub const fn new() -> Self {
        Self {
            skill_levels: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
        }
    }

    /// Create a skill system from existing level and XP maps.
    ///
    /// Used when hydrating from [`AgentState`](emergence_types::AgentState).
    pub const fn from_maps(levels: BTreeMap<String, u32>, xp: BTreeMap<String, u32>) -> Self {
        Self {
            skill_levels: levels,
            skill_xp: xp,
        }
    }

    /// Return the current level for a skill.
    ///
    /// Returns 0 if the agent has never used this skill.
    pub fn get_level(&self, skill: &str) -> u32 {
        self.skill_levels.get(skill).copied().unwrap_or(0)
    }

    /// Return the current XP for a skill.
    ///
    /// Returns 0 if the agent has never used this skill.
    pub fn get_xp(&self, skill: &str) -> u32 {
        self.skill_xp.get(skill).copied().unwrap_or(0)
    }

    /// Return the XP threshold required to advance to the next level.
    ///
    /// Formula: `current_level * 100`. For level 1 to 2: 100 XP, level 2 to
    /// 3: 200 XP, etc. Returns `None` if the skill is at
    /// [`MAX_SKILL_LEVEL`] or if arithmetic overflows.
    pub fn xp_for_next_level(&self, skill: &str) -> Option<u32> {
        let level = self.get_level(skill);
        if level >= MAX_SKILL_LEVEL {
            return None;
        }
        // If level is 0 (skill not started), first level-up is at 100 XP (level 1 * 100).
        // But the skill starts at level 1 on first XP gain, so threshold is 1 * 100 = 100.
        if level == 0 {
            return Some(100);
        }
        level.checked_mul(100)
    }

    /// Add experience points to a skill.
    ///
    /// If the agent has never used this skill, it is initialized at level 1
    /// with 0 XP before applying the gain. If enough XP accumulates, the
    /// skill levels up (possibly multiple times from a single large gain).
    ///
    /// Returns `Some(new_level)` if the skill leveled up, or `None` if no
    /// level change occurred.
    pub fn add_xp(&mut self, skill: &str, amount: u32) -> Result<Option<u32>, AgentError> {
        if amount == 0 {
            return Ok(None);
        }

        // Initialize skill at level 1 if not yet started
        let level_entry = self
            .skill_levels
            .entry(String::from(skill))
            .or_insert(1);

        // If already at max level, no XP tracking needed
        if *level_entry >= MAX_SKILL_LEVEL {
            return Ok(None);
        }

        let xp_entry = self
            .skill_xp
            .entry(String::from(skill))
            .or_insert(0);

        *xp_entry = xp_entry.checked_add(amount).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: format!("XP overflow for skill {skill}"),
            }
        })?;

        // Check for level-ups (loop handles multiple level-ups from large XP gains)
        let original_level = *level_entry;
        loop {
            if *level_entry >= MAX_SKILL_LEVEL {
                // At max level, zero out remaining XP
                *xp_entry = 0;
                break;
            }

            let threshold = level_entry.checked_mul(100).ok_or_else(|| {
                AgentError::ArithmeticOverflow {
                    context: format!("level-up threshold overflow for skill {skill}"),
                }
            })?;

            if *xp_entry >= threshold {
                *xp_entry = xp_entry.checked_sub(threshold).ok_or_else(|| {
                    AgentError::ArithmeticOverflow {
                        context: format!("XP subtraction overflow for skill {skill}"),
                    }
                })?;
                *level_entry = level_entry.checked_add(1).ok_or_else(|| {
                    AgentError::ArithmeticOverflow {
                        context: format!("level increment overflow for skill {skill}"),
                    }
                })?;
            } else {
                break;
            }
        }

        if *level_entry > original_level {
            Ok(Some(*level_entry))
        } else {
            Ok(None)
        }
    }

    /// Return an immutable reference to the skill levels map.
    pub const fn skill_levels(&self) -> &BTreeMap<String, u32> {
        &self.skill_levels
    }

    /// Return an immutable reference to the skill XP map.
    pub const fn skill_xp(&self) -> &BTreeMap<String, u32> {
        &self.skill_xp
    }

    /// Sync this skill system's state into an [`AgentState`](emergence_types::AgentState).
    ///
    /// Copies levels and XP maps into the agent state so both stay consistent.
    pub fn sync_to_agent_state(&self, state: &mut emergence_types::AgentState) {
        state.skills.clone_from(&self.skill_levels);
        state.skill_xp.clone_from(&self.skill_xp);
    }
}

impl Default for SkillSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Skill Effects (Task 3.5.2)
// ---------------------------------------------------------------------------

/// Skill effect calculations for action outcome modification.
///
/// These functions compute modified values based on skill level per
/// `agent-system.md` section 3.6. Integer arithmetic is used where possible;
/// [`rust_decimal::Decimal`] is used when fractional precision matters.
pub mod effects {
    use rust_decimal::Decimal;

    /// Compute the modified gathering yield.
    ///
    /// Formula: `base_yield + (skill_level * 0.5)`
    ///
    /// In integer arithmetic: `base_yield + skill_level / 2`. This means a
    /// level 4 gatherer gets `base + 2` extra units, and odd levels round
    /// down (level 3 = +1, level 5 = +2).
    ///
    /// Returns `None` on arithmetic overflow.
    pub fn gathering_yield(base_yield: u32, skill_level: u32) -> Option<u32> {
        let bonus = skill_level.checked_div(2)?;
        base_yield.checked_add(bonus)
    }

    /// Compute the modified mining yield.
    ///
    /// Formula: `base_yield + (skill_level * 0.5)`
    ///
    /// Same scaling as gathering yield. A level 4 miner gets `base + 2`
    /// extra ore units; odd levels round down.
    ///
    /// Returns `None` on arithmetic overflow.
    pub fn mining_yield(base_yield: u32, skill_level: u32) -> Option<u32> {
        let bonus = skill_level.checked_div(2)?;
        base_yield.checked_add(bonus)
    }

    /// Compute the modified building time.
    ///
    /// Formula: `base_time / (1 + skill_level * 0.1)`
    ///
    /// Uses [`Decimal`] for precise division. The result is truncated to
    /// the nearest whole tick (minimum 1).
    ///
    /// Returns `None` on arithmetic overflow or conversion failure.
    pub fn building_time(base_time: u32, skill_level: u32) -> Option<u32> {
        let base = Decimal::from(base_time);
        let level = Decimal::from(skill_level);
        let one_tenth = Decimal::new(1, 1); // 0.1
        let divisor = Decimal::ONE.checked_add(level.checked_mul(one_tenth)?)?;

        // Avoid division by zero (should never happen since divisor >= 1.0)
        if divisor <= Decimal::ZERO {
            return None;
        }

        let result = base.checked_div(divisor)?;
        let truncated = result.trunc();

        // Convert to u32 safely via string parsing
        let val = truncated.normalize().to_string().parse::<i64>().ok()?;
        if val < 1 {
            // Minimum 1 tick
            Some(1)
        } else {
            u32::try_from(val).ok()
        }
    }

    /// Compute the modified teaching success rate.
    ///
    /// Formula: `base_rate + (skill_level * 0.05)`, capped at 0.99.
    ///
    /// `base_rate_pct` and the result are expressed as percentages (0--100).
    /// The per-level bonus is 5 percentage points. The result is capped at 99%.
    ///
    /// Returns `None` on arithmetic overflow.
    pub fn teaching_success_pct(base_rate_pct: u32, skill_level: u32) -> Option<u32> {
        let bonus = skill_level.checked_mul(5)?;
        let total = base_rate_pct.checked_add(bonus)?;
        Some(total.min(99))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // -------------------------------------------------------------------
        // Gathering yield
        // -------------------------------------------------------------------

        #[test]
        fn gathering_yield_no_skill() {
            assert_eq!(gathering_yield(3, 0), Some(3));
        }

        #[test]
        fn gathering_yield_level_1() {
            // 3 + 1/2 = 3 + 0 = 3
            assert_eq!(gathering_yield(3, 1), Some(3));
        }

        #[test]
        fn gathering_yield_level_2() {
            // 3 + 2/2 = 3 + 1 = 4
            assert_eq!(gathering_yield(3, 2), Some(4));
        }

        #[test]
        fn gathering_yield_level_4() {
            // 3 + 4/2 = 3 + 2 = 5
            assert_eq!(gathering_yield(3, 4), Some(5));
        }

        #[test]
        fn gathering_yield_level_10() {
            // 3 + 10/2 = 3 + 5 = 8
            assert_eq!(gathering_yield(3, 10), Some(8));
        }

        #[test]
        fn gathering_yield_max_level() {
            // 3 + 20/2 = 3 + 10 = 13
            assert_eq!(gathering_yield(3, 20), Some(13));
        }

        // -------------------------------------------------------------------
        // Mining yield
        // -------------------------------------------------------------------

        #[test]
        fn mining_yield_no_skill() {
            assert_eq!(mining_yield(2, 0), Some(2));
        }

        #[test]
        fn mining_yield_level_4() {
            // 2 + 4/2 = 2 + 2 = 4
            assert_eq!(mining_yield(2, 4), Some(4));
        }

        #[test]
        fn mining_yield_level_10() {
            // 2 + 10/2 = 2 + 5 = 7
            assert_eq!(mining_yield(2, 10), Some(7));
        }

        // -------------------------------------------------------------------
        // Building time
        // -------------------------------------------------------------------

        #[test]
        fn building_time_no_skill() {
            // 10 / (1 + 0 * 0.1) = 10 / 1.0 = 10
            assert_eq!(building_time(10, 0), Some(10));
        }

        #[test]
        fn building_time_level_1() {
            // 10 / (1 + 1 * 0.1) = 10 / 1.1 = 9.09 -> 9
            assert_eq!(building_time(10, 1), Some(9));
        }

        #[test]
        fn building_time_level_5() {
            // 10 / (1 + 5 * 0.1) = 10 / 1.5 = 6.66 -> 6
            assert_eq!(building_time(10, 5), Some(6));
        }

        #[test]
        fn building_time_level_10() {
            // 10 / (1 + 10 * 0.1) = 10 / 2.0 = 5
            assert_eq!(building_time(10, 10), Some(5));
        }

        #[test]
        fn building_time_level_20() {
            // 10 / (1 + 20 * 0.1) = 10 / 3.0 = 3.33 -> 3
            assert_eq!(building_time(10, 20), Some(3));
        }

        #[test]
        fn building_time_minimum_one() {
            // 1 / (1 + 20 * 0.1) = 1 / 3.0 = 0.33 -> clamped to 1
            assert_eq!(building_time(1, 20), Some(1));
        }

        // -------------------------------------------------------------------
        // Teaching success
        // -------------------------------------------------------------------

        #[test]
        fn teaching_success_no_skill() {
            // 80 + 0 * 5 = 80
            assert_eq!(teaching_success_pct(80, 0), Some(80));
        }

        #[test]
        fn teaching_success_level_1() {
            // 80 + 1 * 5 = 85
            assert_eq!(teaching_success_pct(80, 1), Some(85));
        }

        #[test]
        fn teaching_success_level_3() {
            // 80 + 3 * 5 = 95
            assert_eq!(teaching_success_pct(80, 3), Some(95));
        }

        #[test]
        fn teaching_success_capped_at_99() {
            // 80 + 10 * 5 = 130, capped at 99
            assert_eq!(teaching_success_pct(80, 10), Some(99));
        }

        #[test]
        fn teaching_success_high_skill() {
            // 80 + 20 * 5 = 180, capped at 99
            assert_eq!(teaching_success_pct(80, 20), Some(99));
        }

        #[test]
        fn teaching_success_low_base() {
            // 50 + 4 * 5 = 70
            assert_eq!(teaching_success_pct(50, 4), Some(70));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // SkillSystem basics
    // -----------------------------------------------------------------------

    #[test]
    fn new_system_is_empty() {
        let ss = SkillSystem::new();
        assert_eq!(ss.get_level("gathering"), 0);
        assert_eq!(ss.get_xp("gathering"), 0);
    }

    #[test]
    fn add_xp_initializes_skill_at_level_1() {
        let mut ss = SkillSystem::new();
        let result = ss.add_xp("gathering", 10);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), None); // No level-up yet
        assert_eq!(ss.get_level("gathering"), 1);
        assert_eq!(ss.get_xp("gathering"), 10);
    }

    #[test]
    fn add_zero_xp_does_nothing() {
        let mut ss = SkillSystem::new();
        let result = ss.add_xp("gathering", 0);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), None);
        assert_eq!(ss.get_level("gathering"), 0);
    }

    #[test]
    fn level_up_at_threshold() {
        let mut ss = SkillSystem::new();
        // Add 100 XP to trigger level 1 -> 2
        let result = ss.add_xp("gathering", 100);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(2)); // Leveled up to 2
        assert_eq!(ss.get_level("gathering"), 2);
        assert_eq!(ss.get_xp("gathering"), 0); // XP spent on level-up
    }

    #[test]
    fn level_up_with_remainder() {
        let mut ss = SkillSystem::new();
        // Add 150 XP: 100 for level-up, 50 remaining
        let result = ss.add_xp("gathering", 150);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(2));
        assert_eq!(ss.get_level("gathering"), 2);
        assert_eq!(ss.get_xp("gathering"), 50);
    }

    #[test]
    fn multiple_level_ups_single_add() {
        let mut ss = SkillSystem::new();
        // Level 1->2 costs 100, level 2->3 costs 200. Total: 300.
        let result = ss.add_xp("gathering", 300);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(3));
        assert_eq!(ss.get_level("gathering"), 3);
        assert_eq!(ss.get_xp("gathering"), 0);
    }

    #[test]
    fn incremental_xp_accumulation() {
        let mut ss = SkillSystem::new();

        // 10 additions of 10 XP = 100 total, should level up
        for i in 1..=9 {
            let result = ss.add_xp("gathering", 10);
            assert!(result.is_ok());
            assert_eq!(result.ok().flatten(), None, "iteration {i}");
        }
        assert_eq!(ss.get_level("gathering"), 1);
        assert_eq!(ss.get_xp("gathering"), 90);

        // 10th addition triggers level-up
        let result = ss.add_xp("gathering", 10);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(2));
        assert_eq!(ss.get_level("gathering"), 2);
        assert_eq!(ss.get_xp("gathering"), 0);
    }

    #[test]
    fn max_level_cap() {
        let mut ss = SkillSystem::new();

        // Fast-track to level 20 by giving massive XP.
        // Sum of thresholds: 1*100 + 2*100 + ... + 19*100 = 100 * (1+2+...+19)
        // = 100 * 190 = 19000
        let result = ss.add_xp("gathering", 19000);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(20));
        assert_eq!(ss.get_level("gathering"), 20);
        assert_eq!(ss.get_xp("gathering"), 0);

        // Adding more XP at max level is a no-op
        let result = ss.add_xp("gathering", 500);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), None);
        assert_eq!(ss.get_level("gathering"), 20);
    }

    #[test]
    fn xp_for_next_level_thresholds() {
        let mut ss = SkillSystem::new();
        // Not started: threshold is 100 (first level-up from 1 to 2)
        assert_eq!(ss.xp_for_next_level("gathering"), Some(100));

        let _ = ss.add_xp("gathering", 10);
        // Level 1: threshold = 1 * 100 = 100
        assert_eq!(ss.xp_for_next_level("gathering"), Some(100));

        let _ = ss.add_xp("gathering", 90);
        // Level 2: threshold = 2 * 100 = 200
        assert_eq!(ss.xp_for_next_level("gathering"), Some(200));
    }

    #[test]
    fn xp_for_next_level_at_max() {
        let mut ss = SkillSystem::new();
        let _ = ss.add_xp("gathering", 19000);
        assert_eq!(ss.xp_for_next_level("gathering"), None);
    }

    #[test]
    fn multiple_skills_independent() {
        let mut ss = SkillSystem::new();

        let _ = ss.add_xp("gathering", 50);
        let _ = ss.add_xp("building", 200);

        assert_eq!(ss.get_level("gathering"), 1);
        assert_eq!(ss.get_xp("gathering"), 50);
        assert_eq!(ss.get_level("building"), 2);
        // 200 total - 100 for L1->2 = 100 remaining, which is < 200 for L2->3
        assert_eq!(ss.get_xp("building"), 100);
    }

    #[test]
    fn from_maps_hydration() {
        let mut levels = BTreeMap::new();
        levels.insert(String::from("gathering"), 5);
        let mut xp = BTreeMap::new();
        xp.insert(String::from("gathering"), 250);

        let ss = SkillSystem::from_maps(levels, xp);
        assert_eq!(ss.get_level("gathering"), 5);
        assert_eq!(ss.get_xp("gathering"), 250);
    }

    #[test]
    fn sync_to_agent_state() {
        let mut ss = SkillSystem::new();
        let _ = ss.add_xp("gathering", 150);
        let _ = ss.add_xp("building", 50);

        let mut state = emergence_types::AgentState {
            agent_id: emergence_types::AgentId::new(),
            energy: 80,
            health: 100,
            hunger: 0,
            age: 0,
            born_at_tick: 0,
            location_id: emergence_types::LocationId::new(),
            destination_id: None,
            travel_progress: 0,
            inventory: BTreeMap::new(),
            carry_capacity: 50,
            knowledge: std::collections::BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        };

        ss.sync_to_agent_state(&mut state);

        assert_eq!(state.skills.get("gathering").copied(), Some(2));
        assert_eq!(state.skill_xp.get("gathering").copied(), Some(50));
        assert_eq!(state.skills.get("building").copied(), Some(1));
        assert_eq!(state.skill_xp.get("building").copied(), Some(50));
    }

    // -----------------------------------------------------------------------
    // Level-up math edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn level_2_to_3_needs_200() {
        let mut ss = SkillSystem::new();
        // Get to level 2
        let _ = ss.add_xp("gathering", 100);
        assert_eq!(ss.get_level("gathering"), 2);

        // Add 199: not enough for level 3
        let result = ss.add_xp("gathering", 199);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), None);
        assert_eq!(ss.get_level("gathering"), 2);
        assert_eq!(ss.get_xp("gathering"), 199);

        // Add 1 more: hits 200 threshold
        let result = ss.add_xp("gathering", 1);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(3));
        assert_eq!(ss.get_level("gathering"), 3);
        assert_eq!(ss.get_xp("gathering"), 0);
    }

    #[test]
    fn building_multiple_level_ups_with_remainder() {
        let mut ss = SkillSystem::new();
        // Level 1->2: 100, Level 2->3: 200, Level 3->4: 300. Total: 600.
        // Give 650: should reach level 4 with 50 XP remaining.
        let result = ss.add_xp("building", 650);
        assert!(result.is_ok());
        assert_eq!(result.ok().flatten(), Some(4));
        assert_eq!(ss.get_level("building"), 4);
        assert_eq!(ss.get_xp("building"), 50);
    }

    // -----------------------------------------------------------------------
    // SKILL_NAMES and MAX_SKILL_LEVEL constants
    // -----------------------------------------------------------------------

    #[test]
    fn skill_names_count() {
        assert_eq!(SKILL_NAMES.len(), 10);
    }

    #[test]
    fn max_skill_level_is_20() {
        assert_eq!(MAX_SKILL_LEVEL, 20);
    }
}
