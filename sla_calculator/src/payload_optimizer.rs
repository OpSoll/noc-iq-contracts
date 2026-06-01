//! SC-W5-037 – Event payload size optimization without semantic loss.
//!
//! This module provides optimized event payload encoding that reduces
//! on-chain storage and gas costs while preserving full semantic meaning.
//!
//! Optimization strategies:
//! 1. Derive `payment_type` from `status` — "viol" → "pen", "met" → "rew"
//! 2. Use compact field ordering to minimize Soroban encoding overhead
//! 3. Omit fields that are fully derivable from other event data

use soroban_sdk::{symbol_short, Symbol};

/// Derive payment type from SLA status.
/// Returns "pen" for violation, "rew" for met.
pub fn derive_payment_type(status: &Symbol) -> Symbol {
    if *status == symbol_short!("viol") {
        symbol_short!("pen")
    } else {
        symbol_short!("rew")
    }
}

/// Returns true if the status is a valid SLA outcome symbol.
pub fn is_valid_status(status: &Symbol) -> bool {
    *status == symbol_short!("met") || *status == symbol_short!("viol")
}

/// Returns true if the payment type is consistent with the given status.
pub fn is_consistent_payment(status: &Symbol, payment_type: &Symbol) -> bool {
    derive_payment_type(status) == *payment_type
}

/// Returns true if the rating is a valid tier symbol.
pub fn is_valid_rating(rating: &Symbol) -> bool {
    *rating == symbol_short!("top")
        || *rating == symbol_short!("excel")
        || *rating == symbol_short!("good")
        || *rating == symbol_short!("poor")
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_derive_payment_from_met() {
        assert_eq!(derive_payment_type(&symbol_short!("met")), symbol_short!("rew"));
    }

    #[test]
    fn test_derive_payment_from_viol() {
        assert_eq!(derive_payment_type(&symbol_short!("viol")), symbol_short!("pen"));
    }

    #[test]
    fn test_valid_statuses() {
        assert!(is_valid_status(&symbol_short!("met")));
        assert!(is_valid_status(&symbol_short!("viol")));
        assert!(!is_valid_status(&symbol_short!("unknown")));
    }

    #[test]
    fn test_consistent_payment() {
        assert!(is_consistent_payment(&symbol_short!("met"), &symbol_short!("rew")));
        assert!(is_consistent_payment(&symbol_short!("viol"), &symbol_short!("pen")));
        assert!(!is_consistent_payment(&symbol_short!("met"), &symbol_short!("pen")));
    }

    #[test]
    fn test_valid_ratings() {
        assert!(is_valid_rating(&symbol_short!("top")));
        assert!(is_valid_rating(&symbol_short!("excel")));
        assert!(is_valid_rating(&symbol_short!("good")));
        assert!(is_valid_rating(&symbol_short!("poor")));
        assert!(!is_valid_rating(&symbol_short!("unknown")));
    }
}
