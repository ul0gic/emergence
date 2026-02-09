//! Fuzzy resource representation for agent perception.
//!
//! Agents do not see exact resource quantities. Instead, they see fuzzy
//! buckets that provide an approximate sense of availability. This prevents
//! agents from making perfectly optimal decisions and encourages emergent
//! exploration and communication.
//!
//! Per `data-schemas.md` section 5.3, the fuzzy thresholds are:
//! - 0: "none"
//! - 1--5: "scarce"
//! - 6--15: "limited"
//! - 16--30: "moderate"
//! - 31--60: "abundant"
//! - 61+: "plentiful"

/// Convert an exact resource quantity to a fuzzy perception string.
///
/// The returned string is one of: "none", "scarce", "limited", "moderate",
/// "abundant", or "plentiful".
pub const fn fuzzy_quantity(available: u32) -> &'static str {
    if available == 0 {
        "none"
    } else if available <= 5 {
        "scarce"
    } else if available <= 15 {
        "limited"
    } else if available <= 30 {
        "moderate"
    } else if available <= 60 {
        "abundant"
    } else {
        "plentiful"
    }
}

/// Convert a fuzzy string back to a representative midpoint value.
///
/// This is useful for tests and debugging. Returns `None` if the string
/// is not a recognized fuzzy category.
pub fn midpoint_for_fuzzy(label: &str) -> Option<u32> {
    match label {
        "none" => Some(0),
        "scarce" => Some(3),
        "limited" => Some(10),
        "moderate" => Some(23),
        "abundant" => Some(45),
        "plentiful" => Some(80),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_none() {
        assert_eq!(fuzzy_quantity(0), "none");
    }

    #[test]
    fn scarce_range() {
        assert_eq!(fuzzy_quantity(1), "scarce");
        assert_eq!(fuzzy_quantity(3), "scarce");
        assert_eq!(fuzzy_quantity(5), "scarce");
    }

    #[test]
    fn limited_range() {
        assert_eq!(fuzzy_quantity(6), "limited");
        assert_eq!(fuzzy_quantity(10), "limited");
        assert_eq!(fuzzy_quantity(15), "limited");
    }

    #[test]
    fn moderate_range() {
        assert_eq!(fuzzy_quantity(16), "moderate");
        assert_eq!(fuzzy_quantity(25), "moderate");
        assert_eq!(fuzzy_quantity(30), "moderate");
    }

    #[test]
    fn abundant_range() {
        assert_eq!(fuzzy_quantity(31), "abundant");
        assert_eq!(fuzzy_quantity(45), "abundant");
        assert_eq!(fuzzy_quantity(60), "abundant");
    }

    #[test]
    fn plentiful_range() {
        assert_eq!(fuzzy_quantity(61), "plentiful");
        assert_eq!(fuzzy_quantity(100), "plentiful");
        assert_eq!(fuzzy_quantity(1000), "plentiful");
    }

    #[test]
    fn midpoint_round_trip() {
        let labels = ["none", "scarce", "limited", "moderate", "abundant", "plentiful"];
        for label in labels {
            let mid = midpoint_for_fuzzy(label);
            assert!(mid.is_some(), "midpoint not found for {label}");
            let result = fuzzy_quantity(mid.unwrap_or(0));
            assert_eq!(result, label, "round-trip failed for {label}");
        }
    }

    #[test]
    fn unknown_label_returns_none() {
        assert_eq!(midpoint_for_fuzzy("massive"), None);
        assert_eq!(midpoint_for_fuzzy(""), None);
    }
}
