use super::ViewCoordinates2D;

impl std::ops::Deref for ViewCoordinates2D {
    type Target = [u8; 2];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ViewCoordinates2D {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
