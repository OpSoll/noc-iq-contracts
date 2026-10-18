use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

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

/// View returning the current rating band for a site, given its SLA score.
///
/// The band is derived by [`SLARatingClassifier::classify_sla`], so the view
/// and the classifier can never disagree about where a score belongs.
pub fn rating_band(_env: &Env, score: u32) -> SLARating {
    SLARatingClassifier::classify_sla(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    /// Thresholds documented on [`SLARatingClassifier::classify_sla`]:
    /// Top >= 99, Excel >= 95, Good >= 90, everything else Viol.
    #[test]
    fn transitions_occur_exactly_at_documented_boundaries() {
        // Each score sits one point on either side of a boundary.
        assert_eq!(SLARatingClassifier::classify_sla(99), SLARating::Top);
        assert_eq!(SLARatingClassifier::classify_sla(98), SLARating::Excel);

        assert_eq!(SLARatingClassifier::classify_sla(95), SLARating::Excel);
        assert_eq!(SLARatingClassifier::classify_sla(94), SLARating::Good);

        assert_eq!(SLARatingClassifier::classify_sla(90), SLARating::Good);
        assert_eq!(SLARatingClassifier::classify_sla(89), SLARating::Viol);
    }

    #[test]
    fn band_never_changes_inside_a_band() {
        // Sweeping every score inside a band must yield the same rating, so a
        // site cannot oscillate between bands without crossing a boundary.
        for score in 99..=100u32 {
            assert_eq!(SLARatingClassifier::classify_sla(score), SLARating::Top);
        }
        for score in 95..=98u32 {
            assert_eq!(SLARatingClassifier::classify_sla(score), SLARating::Excel);
        }
        for score in 90..=94u32 {
            assert_eq!(SLARatingClassifier::classify_sla(score), SLARating::Good);
        }
        for score in 0..=89u32 {
            assert_eq!(SLARatingClassifier::classify_sla(score), SLARating::Viol);
        }
    }

    #[test]
    fn view_returns_the_classifier_band_for_every_boundary() {
        let env = Env::default();
        let scores = [0u32, 1, 89, 90, 94, 95, 98, 99, 100, 1_000, u32::MAX];
        for score in scores {
            assert_eq!(
                rating_band(&env, score),
                SLARatingClassifier::classify_sla(score),
                "view disagreed with classifier at score {score}"
            );
        }
    }

    #[test]
    fn each_band_exposes_its_documented_symbol() {
        // Consumers read bands as symbols, so the mapping is pinned too.
        assert_eq!(
            SLARatingClassifier::to_symbol(&SLARating::Top),
            symbol_short!("top")
        );
        assert_eq!(
            SLARatingClassifier::to_symbol(&SLARating::Excel),
            symbol_short!("excel")
        );
        assert_eq!(
            SLARatingClassifier::to_symbol(&SLARating::Good),
            symbol_short!("good")
        );
        assert_eq!(
            SLARatingClassifier::to_symbol(&SLARating::Viol),
            symbol_short!("viol")
        );
    }
}
