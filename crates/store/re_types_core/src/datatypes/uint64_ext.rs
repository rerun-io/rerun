use super::UInt64;

impl std::ops::Deref for UInt64 {
    type Target = u64;

    #[inline]
    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl std::ops::DerefMut for UInt64 {
    #[inline]
    fn deref_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}
