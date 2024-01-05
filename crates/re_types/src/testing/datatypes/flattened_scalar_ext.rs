use super::FlattenedScalar;

impl crate::SizeBytes for FlattenedScalar {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { value: _ } = self;
        0
    }
}
