use super::ViewCoordinates;

impl std::ops::Deref for ViewCoordinates {
    type Target = [u8; 3];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ViewCoordinates {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
