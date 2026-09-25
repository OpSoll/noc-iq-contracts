use soroban_sdk::{contracttype, Env, Symbol, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SLARating {
    Top,
    Excel,
    Good,
    Viol,
}

pub struct SLARatingClassifier;

impl SLARatingClassifier {
    /// Categorizes an SLA score or uptime percentage into standardized rating symbols.
    pub fn classify_sla(score: u32) -> SLARating {
        if score >= 99 {
            SLARating::Top
        } else if score >= 95 {
            SLARating::Excel
        } else if score >= 90 {
            SLARating::Good
        } else {
            SLARating::Viol
        }
    }

    /// Converts the rating to a Soroban short symbol for on-chain events or storage.
    pub fn to_symbol(rating: &SLARating) -> Symbol {
        match rating {
            SLARating::Top => symbol_short!("top"),
            SLARating::Excel => symbol_short!("excel"),
            SLARating::Good => symbol_short!("good"),
            SLARating::Viol => symbol_short!("viol"),
        }
    }
}

