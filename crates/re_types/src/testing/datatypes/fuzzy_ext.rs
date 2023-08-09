use super::{AffixFuzzer3, AffixFuzzer4};

impl Default for AffixFuzzer3 {
    fn default() -> Self {
        Self::Radians(Some(0.0))
    }
}

impl Default for AffixFuzzer4 {
    fn default() -> Self {
        Self::SingleRequired(Default::default())
    }
}
