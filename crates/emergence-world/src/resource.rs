//! Resource regeneration logic for location-bound resource nodes.
//!
//! Each [`ResourceNode`] at a location has a current `available` quantity, a
//! `regen_per_tick` rate, and a `max_capacity` ceiling. Regeneration runs
//! during the World Wake phase of the tick cycle and is capped so that
//! `available` never exceeds `max_capacity`.
//!
//! Season modifiers adjust the effective regeneration rate:
//! - Spring: +25%
//! - Summer: normal
//! - Autumn: -25%
//! - Winter: -75%

use emergence_types::{ResourceNode, Season};

use crate::error::WorldError;

/// Apply one tick of regeneration to a [`ResourceNode`], respecting the
/// seasonal modifier.
///
/// Returns the number of units actually regenerated (may be zero if the
/// node is already at capacity or the seasonal rate rounds to zero).
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] if checked arithmetic fails.
pub fn regenerate(node: &mut ResourceNode, season: Season) -> Result<u32, WorldError> {
    if node.available >= node.max_capacity {
        return Ok(0);
    }

    let effective_regen = seasonal_regen(node.regen_per_tick, season)?;

    let headroom = node
        .max_capacity
        .checked_sub(node.available)
        .ok_or(WorldError::ArithmeticOverflow)?;

    let added = effective_regen.min(headroom);
    node.available = node
        .available
        .checked_add(added)
        .ok_or(WorldError::ArithmeticOverflow)?;

    Ok(added)
}

/// Calculate the seasonal regeneration rate from a base rate.
///
/// The modifier is applied as integer arithmetic to avoid floating-point:
/// - Spring: `base * 5 / 4` (+25%)
/// - Summer: `base` (no change)
/// - Autumn: `base * 3 / 4` (-25%)
/// - Winter: `base / 4`     (-75%)
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] if checked arithmetic fails.
fn seasonal_regen(base: u32, season: Season) -> Result<u32, WorldError> {
    match season {
        Season::Spring => base
            .checked_mul(5)
            .and_then(|v| v.checked_div(4))
            .ok_or(WorldError::ArithmeticOverflow),
        Season::Summer => Ok(base),
        Season::Autumn => base
            .checked_mul(3)
            .and_then(|v| v.checked_div(4))
            .ok_or(WorldError::ArithmeticOverflow),
        Season::Winter => base.checked_div(4).ok_or(WorldError::ArithmeticOverflow),
    }
}

/// Deduct a quantity from a resource node, returning the actual amount taken.
///
/// If the node has fewer units than requested, the entire remaining amount
/// is taken. Returns the actual units removed.
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] if checked arithmetic fails.
pub fn harvest(node: &mut ResourceNode, requested: u32) -> Result<u32, WorldError> {
    let taken = requested.min(node.available);
    node.available = node
        .available
        .checked_sub(taken)
        .ok_or(WorldError::ArithmeticOverflow)?;
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use emergence_types::Resource;

    use super::*;

    fn make_node(available: u32, regen: u32, max: u32) -> ResourceNode {
        ResourceNode {
            resource: Resource::Wood,
            available,
            regen_per_tick: regen,
            max_capacity: max,
        }
    }

    #[test]
    fn regen_summer_normal() {
        let mut node = make_node(40, 10, 100);
        let added = regenerate(&mut node, Season::Summer);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(10));
        assert_eq!(node.available, 50);
    }

    #[test]
    fn regen_capped_at_max() {
        let mut node = make_node(95, 10, 100);
        let added = regenerate(&mut node, Season::Summer);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(5));
        assert_eq!(node.available, 100);
    }

    #[test]
    fn regen_already_full() {
        let mut node = make_node(100, 10, 100);
        let added = regenerate(&mut node, Season::Summer);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(0));
        assert_eq!(node.available, 100);
    }

    #[test]
    fn regen_spring_bonus() {
        // Spring: 10 * 5 / 4 = 12 (integer)
        let mut node = make_node(0, 10, 100);
        let added = regenerate(&mut node, Season::Spring);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(12));
        assert_eq!(node.available, 12);
    }

    #[test]
    fn regen_autumn_reduction() {
        // Autumn: 10 * 3 / 4 = 7 (integer)
        let mut node = make_node(0, 10, 100);
        let added = regenerate(&mut node, Season::Autumn);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(7));
        assert_eq!(node.available, 7);
    }

    #[test]
    fn regen_winter_severe_reduction() {
        // Winter: 10 / 4 = 2 (integer)
        let mut node = make_node(0, 10, 100);
        let added = regenerate(&mut node, Season::Winter);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(2));
        assert_eq!(node.available, 2);
    }

    #[test]
    fn regen_winter_rounds_to_zero() {
        // Winter: 3 / 4 = 0 (integer division)
        let mut node = make_node(0, 3, 100);
        let added = regenerate(&mut node, Season::Winter);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(0));
        assert_eq!(node.available, 0);
    }

    #[test]
    fn regen_zero_base_rate() {
        // Stone nodes typically have regen_per_tick = 0
        let mut node = make_node(8, 0, 8);
        let added = regenerate(&mut node, Season::Summer);
        assert!(added.is_ok());
        assert_eq!(added.ok(), Some(0));
        assert_eq!(node.available, 8);
    }

    #[test]
    fn harvest_full_amount() {
        let mut node = make_node(50, 5, 100);
        let taken = harvest(&mut node, 10);
        assert!(taken.is_ok());
        assert_eq!(taken.ok(), Some(10));
        assert_eq!(node.available, 40);
    }

    #[test]
    fn harvest_partial_when_scarce() {
        let mut node = make_node(3, 5, 100);
        let taken = harvest(&mut node, 10);
        assert!(taken.is_ok());
        assert_eq!(taken.ok(), Some(3));
        assert_eq!(node.available, 0);
    }

    #[test]
    fn harvest_from_empty_node() {
        let mut node = make_node(0, 5, 100);
        let taken = harvest(&mut node, 10);
        assert!(taken.is_ok());
        assert_eq!(taken.ok(), Some(0));
        assert_eq!(node.available, 0);
    }
}
