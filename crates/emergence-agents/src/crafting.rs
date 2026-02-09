//! Crafting recipes for the workshop.
//!
//! Defines the static recipe table mapping craftable output resources to their
//! input materials and required knowledge. Used by the `craft` action handler
//! during the Resolution phase.
//!
//! See `world-engine.md` section 7.1 (Advanced Actions) and `data-schemas.md`
//! section 7.2 for recipe definitions.

use std::collections::BTreeMap;

use emergence_types::Resource;

// ---------------------------------------------------------------------------
// CraftRecipe
// ---------------------------------------------------------------------------

/// A single crafting recipe: inputs required and knowledge prerequisite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftRecipe {
    /// The resource produced by this recipe.
    pub output: Resource,
    /// How many units of the output are produced per craft.
    pub output_quantity: u32,
    /// Input materials consumed (resource -> quantity).
    pub inputs: BTreeMap<Resource, u32>,
    /// The knowledge concept required to use this recipe.
    pub required_knowledge: &'static str,
}

// ---------------------------------------------------------------------------
// Recipe Table
// ---------------------------------------------------------------------------

/// Look up the crafting recipe for the given output resource.
///
/// Returns `None` if no recipe exists for that resource (i.e. it is not
/// craftable at a workshop).
///
/// Recipes per `world-engine.md` section 7.1:
/// - [`Resource::Tool`]: 3 wood + 2 stone, requires `"basic_tools"`
/// - [`Resource::ToolAdvanced`]: 2 metal + 1 wood, requires `"metalworking"`
/// - [`Resource::Medicine`]: 3 `FoodBerry` + 1 water, requires `"basic_medicine"`
pub fn recipe_for(output: Resource) -> Option<CraftRecipe> {
    match output {
        Resource::Tool => Some(CraftRecipe {
            output: Resource::Tool,
            output_quantity: 1,
            inputs: BTreeMap::from([
                (Resource::Wood, 3),
                (Resource::Stone, 2),
            ]),
            required_knowledge: "basic_tools",
        }),
        Resource::ToolAdvanced => Some(CraftRecipe {
            output: Resource::ToolAdvanced,
            output_quantity: 1,
            inputs: BTreeMap::from([
                (Resource::Metal, 2),
                (Resource::Wood, 1),
            ]),
            required_knowledge: "metalworking",
        }),
        Resource::Medicine => Some(CraftRecipe {
            output: Resource::Medicine,
            output_quantity: 1,
            inputs: BTreeMap::from([
                (Resource::FoodBerry, 3),
                (Resource::Water, 1),
            ]),
            required_knowledge: "basic_medicine",
        }),
        _ => None,
    }
}

/// Return all valid craftable output resources.
///
/// Used by validation to check whether a craft request targets a valid output.
pub const fn craftable_outputs() -> &'static [Resource] {
    &[Resource::Tool, Resource::ToolAdvanced, Resource::Medicine]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_recipe_correct() {
        let r = recipe_for(Resource::Tool);
        assert!(r.is_some());
        let r = r.unwrap_or_else(|| CraftRecipe {
            output: Resource::Wood,
            output_quantity: 0,
            inputs: BTreeMap::new(),
            required_knowledge: "",
        });
        assert_eq!(r.output, Resource::Tool);
        assert_eq!(r.output_quantity, 1);
        assert_eq!(r.inputs.get(&Resource::Wood).copied(), Some(3));
        assert_eq!(r.inputs.get(&Resource::Stone).copied(), Some(2));
        assert_eq!(r.required_knowledge, "basic_tools");
    }

    #[test]
    fn tool_advanced_recipe_correct() {
        let r = recipe_for(Resource::ToolAdvanced);
        assert!(r.is_some());
        let r = r.unwrap_or_else(|| CraftRecipe {
            output: Resource::Wood,
            output_quantity: 0,
            inputs: BTreeMap::new(),
            required_knowledge: "",
        });
        assert_eq!(r.output, Resource::ToolAdvanced);
        assert_eq!(r.inputs.get(&Resource::Metal).copied(), Some(2));
        assert_eq!(r.inputs.get(&Resource::Wood).copied(), Some(1));
        assert_eq!(r.required_knowledge, "metalworking");
    }

    #[test]
    fn medicine_recipe_correct() {
        let r = recipe_for(Resource::Medicine);
        assert!(r.is_some());
        let r = r.unwrap_or_else(|| CraftRecipe {
            output: Resource::Wood,
            output_quantity: 0,
            inputs: BTreeMap::new(),
            required_knowledge: "",
        });
        assert_eq!(r.output, Resource::Medicine);
        assert_eq!(r.inputs.get(&Resource::FoodBerry).copied(), Some(3));
        assert_eq!(r.inputs.get(&Resource::Water).copied(), Some(1));
        assert_eq!(r.required_knowledge, "basic_medicine");
    }

    #[test]
    fn non_craftable_returns_none() {
        assert!(recipe_for(Resource::Wood).is_none());
        assert!(recipe_for(Resource::Stone).is_none());
        assert!(recipe_for(Resource::Water).is_none());
        assert!(recipe_for(Resource::Ore).is_none());
    }

    #[test]
    fn craftable_outputs_lists_all_recipes() {
        let outputs = craftable_outputs();
        assert_eq!(outputs.len(), 3);
        assert!(outputs.contains(&Resource::Tool));
        assert!(outputs.contains(&Resource::ToolAdvanced));
        assert!(outputs.contains(&Resource::Medicine));
    }
}
