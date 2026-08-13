impl Default for super::NestedUnion {
    #[inline]
    fn default() -> Self {
        Self::SingleRequired(Default::default())
    }
}
