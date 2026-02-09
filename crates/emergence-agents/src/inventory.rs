//! Inventory (wallet) operations for agents.
//!
//! Each agent carries resources subject to a weight limit (`carry_capacity`).
//! This module provides methods for adding, removing, and querying resources
//! with full checked arithmetic -- no silent overflows, no panics.
//!
//! See `agent-system.md` section 3.2 and `data-schemas.md` section 4.3.

use std::collections::BTreeMap;

use emergence_types::Resource;

use crate::error::AgentError;

/// Compute the total weight (sum of all quantities) in an inventory.
///
/// Returns `None` if the sum overflows `u32`.
pub fn total_weight(inventory: &BTreeMap<Resource, u32>) -> Option<u32> {
    let mut total: u32 = 0;
    for qty in inventory.values() {
        total = total.checked_add(*qty)?;
    }
    Some(total)
}

/// Check whether the current load exceeds or equals the carry capacity.
///
/// Returns `None` if the total weight computation overflows.
pub fn is_overloaded(inventory: &BTreeMap<Resource, u32>, carry_capacity: u32) -> Option<bool> {
    let weight = total_weight(inventory)?;
    Some(weight > carry_capacity)
}

/// Check whether the inventory contains at least `amount` of the given resource.
pub fn has_resource(inventory: &BTreeMap<Resource, u32>, resource: Resource, amount: u32) -> bool {
    inventory.get(&resource).copied().unwrap_or(0) >= amount
}

/// Add `amount` units of `resource` to the inventory.
///
/// Fails if the addition would exceed `carry_capacity` or cause a `u32` overflow.
pub fn add_resource(
    inventory: &mut BTreeMap<Resource, u32>,
    carry_capacity: u32,
    resource: Resource,
    amount: u32,
) -> Result<(), AgentError> {
    let current_load = total_weight(inventory).ok_or_else(|| AgentError::ArithmeticOverflow {
        context: String::from("total_weight overflow in add_resource"),
    })?;

    let new_load = current_load.checked_add(amount).ok_or(AgentError::InventoryOverflow {
        resource,
        attempted: amount,
        current_load,
        capacity: carry_capacity,
    })?;

    if new_load > carry_capacity {
        return Err(AgentError::InventoryOverflow {
            resource,
            attempted: amount,
            current_load,
            capacity: carry_capacity,
        });
    }

    let entry = inventory.entry(resource).or_insert(0);
    // This cannot overflow because new_load <= carry_capacity <= u32::MAX
    // and the individual quantity is bounded by the total load.
    *entry = entry.checked_add(amount).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("individual resource quantity overflow"),
        }
    })?;

    Ok(())
}

/// Remove `amount` units of `resource` from the inventory.
///
/// Fails if the agent does not hold enough of the resource.
/// Removes the key entirely if quantity reaches zero.
pub fn remove_resource(
    inventory: &mut BTreeMap<Resource, u32>,
    resource: Resource,
    amount: u32,
) -> Result<(), AgentError> {
    let current = inventory.get(&resource).copied().unwrap_or(0);

    if current < amount {
        return Err(AgentError::InsufficientResource {
            resource,
            requested: amount,
            available: current,
        });
    }

    let remaining = current.checked_sub(amount).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("subtraction underflow in remove_resource"),
        }
    })?;

    if remaining == 0 {
        inventory.remove(&resource);
    } else {
        inventory.insert(resource, remaining);
    }

    Ok(())
}

/// Drain all resources from the inventory, returning them as a new map.
///
/// The inventory is left empty after this call. This is used during
/// death processing when an agent's inventory drops at their location.
pub const fn drain_all(inventory: &mut BTreeMap<Resource, u32>) -> BTreeMap<Resource, u32> {
    let mut dropped = BTreeMap::new();
    core::mem::swap(inventory, &mut dropped);
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_inventory() -> BTreeMap<Resource, u32> {
        BTreeMap::new()
    }

    #[test]
    fn total_weight_empty() {
        assert_eq!(total_weight(&empty_inventory()), Some(0));
    }

    #[test]
    fn total_weight_single_resource() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 10);
        assert_eq!(total_weight(&inv), Some(10));
    }

    #[test]
    fn total_weight_multiple_resources() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 10);
        inv.insert(Resource::Stone, 5);
        inv.insert(Resource::FoodBerry, 3);
        assert_eq!(total_weight(&inv), Some(18));
    }

    #[test]
    fn is_overloaded_within_capacity() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 25);
        assert_eq!(is_overloaded(&inv, 50), Some(false));
    }

    #[test]
    fn is_overloaded_at_capacity() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 50);
        assert_eq!(is_overloaded(&inv, 50), Some(false));
    }

    #[test]
    fn is_overloaded_over_capacity() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 51);
        assert_eq!(is_overloaded(&inv, 50), Some(true));
    }

    #[test]
    fn has_resource_present() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Stone, 10);
        assert!(has_resource(&inv, Resource::Stone, 5));
        assert!(has_resource(&inv, Resource::Stone, 10));
    }

    #[test]
    fn has_resource_insufficient() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Stone, 3);
        assert!(!has_resource(&inv, Resource::Stone, 5));
    }

    #[test]
    fn has_resource_absent() {
        let inv = empty_inventory();
        assert!(!has_resource(&inv, Resource::Wood, 1));
    }

    #[test]
    fn has_resource_zero_requested() {
        let inv = empty_inventory();
        assert!(has_resource(&inv, Resource::Wood, 0));
    }

    #[test]
    fn add_resource_success() {
        let mut inv = empty_inventory();
        assert!(add_resource(&mut inv, 50, Resource::Wood, 10).is_ok());
        assert_eq!(inv.get(&Resource::Wood).copied(), Some(10));
    }

    #[test]
    fn add_resource_stacks() {
        let mut inv = empty_inventory();
        assert!(add_resource(&mut inv, 50, Resource::Wood, 10).is_ok());
        assert!(add_resource(&mut inv, 50, Resource::Wood, 5).is_ok());
        assert_eq!(inv.get(&Resource::Wood).copied(), Some(15));
    }

    #[test]
    fn add_resource_exceeds_capacity() {
        let mut inv = empty_inventory();
        assert!(add_resource(&mut inv, 50, Resource::Wood, 30).is_ok());
        let result = add_resource(&mut inv, 50, Resource::Stone, 25);
        assert!(result.is_err());
        // Inventory should not have changed
        assert_eq!(inv.get(&Resource::Stone), None);
    }

    #[test]
    fn add_resource_exact_capacity() {
        let mut inv = empty_inventory();
        assert!(add_resource(&mut inv, 50, Resource::Wood, 50).is_ok());
        assert_eq!(total_weight(&inv), Some(50));
    }

    #[test]
    fn remove_resource_success() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 10);
        assert!(remove_resource(&mut inv, Resource::Wood, 5).is_ok());
        assert_eq!(inv.get(&Resource::Wood).copied(), Some(5));
    }

    #[test]
    fn remove_resource_exact() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 10);
        assert!(remove_resource(&mut inv, Resource::Wood, 10).is_ok());
        // Key should be removed entirely when quantity hits zero
        assert_eq!(inv.get(&Resource::Wood), None);
    }

    #[test]
    fn remove_resource_insufficient() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 3);
        let result = remove_resource(&mut inv, Resource::Wood, 5);
        assert!(result.is_err());
        // Inventory unchanged on failure
        assert_eq!(inv.get(&Resource::Wood).copied(), Some(3));
    }

    #[test]
    fn remove_resource_absent() {
        let mut inv = empty_inventory();
        let result = remove_resource(&mut inv, Resource::Wood, 1);
        assert!(result.is_err());
    }

    #[test]
    fn drain_all_returns_contents() {
        let mut inv = empty_inventory();
        inv.insert(Resource::Wood, 10);
        inv.insert(Resource::Stone, 5);
        let dropped = drain_all(&mut inv);
        assert_eq!(dropped.get(&Resource::Wood).copied(), Some(10));
        assert_eq!(dropped.get(&Resource::Stone).copied(), Some(5));
        assert!(inv.is_empty());
    }

    #[test]
    fn drain_all_empty_inventory() {
        let mut inv = empty_inventory();
        let dropped = drain_all(&mut inv);
        assert!(dropped.is_empty());
        assert!(inv.is_empty());
    }
}
