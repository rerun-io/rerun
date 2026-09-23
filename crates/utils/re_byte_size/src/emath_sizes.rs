use crate::SizeBytes;

impl SizeBytes for emath::Rangef {
    const IS_POD: bool = true;

    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}
