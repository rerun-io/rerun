use super::AffixFuzzer1;

// TODO(#4690): this should be codegen'd.
impl crate::SizeBytes for AffixFuzzer1 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(v) = self;
        v.heap_size_bytes()
    }
}
