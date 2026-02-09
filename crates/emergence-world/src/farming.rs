//! Farm plot state tracking: planting, growth timers, and harvest readiness.
//!
//! A [`FarmPlot`](emergence_types::StructureType::FarmPlot) structure can hold
//! crops that grow over time. This module tracks per-farm-plot growth state
//! and provides helpers for planting and harvesting.
//!
//! See `world-engine.md` section 7.1 (Advanced Actions) and section 5.2
//! (`FarmPlot` structure).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use emergence_types::StructureId;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default number of ticks for crops to mature after planting.
pub const DEFAULT_GROWTH_TICKS: u64 = 10;

/// Base harvest yield in units of `FoodFarmed`.
pub const BASE_HARVEST_YIELD: u32 = 5;

// ---------------------------------------------------------------------------
// FarmCropState
// ---------------------------------------------------------------------------

/// Growth state of crops on a single farm plot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmCropState {
    /// The tick when crops were planted.
    pub planted_at_tick: u64,
    /// The tick at which crops become mature and harvestable.
    pub mature_at_tick: u64,
}

impl FarmCropState {
    /// Create a new crop state planted at `current_tick`, maturing after
    /// `growth_ticks` ticks.
    ///
    /// Returns `None` on arithmetic overflow.
    pub fn plant(current_tick: u64, growth_ticks: u64) -> Option<Self> {
        let mature_at = current_tick.checked_add(growth_ticks)?;
        Some(Self {
            planted_at_tick: current_tick,
            mature_at_tick: mature_at,
        })
    }

    /// Check whether the crops are mature (ready for harvest).
    pub const fn is_mature(&self, current_tick: u64) -> bool {
        current_tick >= self.mature_at_tick
    }
}

// ---------------------------------------------------------------------------
// FarmRegistry
// ---------------------------------------------------------------------------

/// Registry mapping farm plot structure IDs to their crop growth state.
///
/// Farms without an entry in this registry have no crops planted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmRegistry {
    /// Maps structure ID of each farm plot to its crop state.
    crops: BTreeMap<StructureId, FarmCropState>,
}

impl FarmRegistry {
    /// Create an empty farm registry.
    pub const fn new() -> Self {
        Self {
            crops: BTreeMap::new(),
        }
    }

    /// Plant crops on a farm plot.
    ///
    /// Returns `false` if the farm already has crops planted (must harvest
    /// first) or if arithmetic overflows. Returns `true` on success.
    pub fn plant(
        &mut self,
        farm_id: StructureId,
        current_tick: u64,
        growth_ticks: u64,
    ) -> bool {
        // Cannot plant if crops are already growing
        if self.crops.contains_key(&farm_id) {
            return false;
        }

        if let Some(state) = FarmCropState::plant(current_tick, growth_ticks) {
            self.crops.insert(farm_id, state);
            true
        } else {
            false
        }
    }

    /// Check whether a farm plot has mature crops ready for harvest.
    pub fn is_harvestable(&self, farm_id: StructureId, current_tick: u64) -> bool {
        self.crops
            .get(&farm_id)
            .is_some_and(|state| state.is_mature(current_tick))
    }

    /// Check whether a farm plot has any crops planted (mature or not).
    pub fn has_crops(&self, farm_id: StructureId) -> bool {
        self.crops.contains_key(&farm_id)
    }

    /// Harvest crops from a farm plot, removing the crop state.
    ///
    /// Returns `true` if crops were present and removed, `false` if no crops
    /// were planted on this farm.
    pub fn harvest(&mut self, farm_id: StructureId) -> bool {
        self.crops.remove(&farm_id).is_some()
    }

    /// Return the crop state for a farm plot, if any.
    pub fn get_crop_state(&self, farm_id: StructureId) -> Option<&FarmCropState> {
        self.crops.get(&farm_id)
    }

    /// Remove crop state for a farm that was demolished or collapsed.
    pub fn remove_farm(&mut self, farm_id: StructureId) {
        self.crops.remove(&farm_id);
    }

    /// Return the number of farms with active crops.
    pub fn active_count(&self) -> usize {
        self.crops.len()
    }
}

// ---------------------------------------------------------------------------
// Harvest yield calculation
// ---------------------------------------------------------------------------

/// Compute the harvest yield modified by the agent's farming skill level.
///
/// Formula: `BASE_HARVEST_YIELD + skill_level / 2`
///
/// Returns `None` on arithmetic overflow.
pub fn harvest_yield(skill_level: u32) -> Option<u32> {
    let bonus = skill_level.checked_div(2)?;
    BASE_HARVEST_YIELD.checked_add(bonus)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use emergence_types::StructureId;

    use super::*;

    #[test]
    fn plant_and_check_maturity() {
        let state = FarmCropState::plant(10, 10);
        assert!(state.is_some());
        let state = state.unwrap_or_else(|| FarmCropState {
            planted_at_tick: 0,
            mature_at_tick: 0,
        });
        assert_eq!(state.planted_at_tick, 10);
        assert_eq!(state.mature_at_tick, 20);
        assert!(!state.is_mature(15));
        assert!(!state.is_mature(19));
        assert!(state.is_mature(20));
        assert!(state.is_mature(25));
    }

    #[test]
    fn registry_plant_and_harvest() {
        let mut reg = FarmRegistry::new();
        let farm_id = StructureId::new();

        assert!(reg.plant(farm_id, 5, 10));
        assert!(reg.has_crops(farm_id));
        assert!(!reg.is_harvestable(farm_id, 10));
        assert!(reg.is_harvestable(farm_id, 15));

        assert!(reg.harvest(farm_id));
        assert!(!reg.has_crops(farm_id));
    }

    #[test]
    fn registry_cannot_double_plant() {
        let mut reg = FarmRegistry::new();
        let farm_id = StructureId::new();

        assert!(reg.plant(farm_id, 0, 10));
        assert!(!reg.plant(farm_id, 5, 10)); // Already planted
    }

    #[test]
    fn registry_harvest_empty_returns_false() {
        let mut reg = FarmRegistry::new();
        let farm_id = StructureId::new();
        assert!(!reg.harvest(farm_id));
    }

    #[test]
    fn registry_remove_farm() {
        let mut reg = FarmRegistry::new();
        let farm_id = StructureId::new();
        assert!(reg.plant(farm_id, 0, 10));
        reg.remove_farm(farm_id);
        assert!(!reg.has_crops(farm_id));
    }

    #[test]
    fn harvest_yield_no_skill() {
        assert_eq!(harvest_yield(0), Some(5));
    }

    #[test]
    fn harvest_yield_with_skill() {
        assert_eq!(harvest_yield(4), Some(7)); // 5 + 4/2 = 7
        assert_eq!(harvest_yield(10), Some(10)); // 5 + 10/2 = 10
    }

    #[test]
    fn harvest_yield_odd_skill() {
        assert_eq!(harvest_yield(3), Some(6)); // 5 + 3/2 = 5 + 1 = 6
    }

    #[test]
    fn active_count_tracks_farms() {
        let mut reg = FarmRegistry::new();
        assert_eq!(reg.active_count(), 0);

        let f1 = StructureId::new();
        let f2 = StructureId::new();
        assert!(reg.plant(f1, 0, 10));
        assert_eq!(reg.active_count(), 1);
        assert!(reg.plant(f2, 0, 10));
        assert_eq!(reg.active_count(), 2);

        assert!(reg.harvest(f1));
        assert_eq!(reg.active_count(), 1);
    }
}
