impl Default for super::AffixFuzzer21 {
    #[inline]
    fn default() -> Self {
        // TODO(apache/arrow-rs#7411)
        Self {
            single_half: Default::default(),
            many_halves: vec![].into(),
        }
    }
}
