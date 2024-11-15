use super::UInt32;

impl std::ops::Deref for UInt32 {
    type Target = u32;

    #[inline]
    fn deref(&self) -> &u32 {
        &self.0
    }
}

impl std::ops::DerefMut for UInt32 {
    #[inline]
    fn deref_mut(&mut self) -> &mut u32 {
        &mut self.0
    }
}
