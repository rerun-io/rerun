use super::Bool;

impl std::ops::Deref for Bool {
    type Target = bool;

    #[inline]
    fn deref(&self) -> &bool {
        &self.0
    }
}

impl std::ops::DerefMut for Bool {
    #[inline]
    fn deref_mut(&mut self) -> &mut bool {
        &mut self.0
    }
}
