use super::Cylinders3D;

impl Cylinders3D {
    /// Creates a new [`Cylinders3D`] with the given axis-aligned lengths and radii.
    ///
    /// For multiple cylinders, you should generally follow this with
    /// [`Cylinders3D::with_centers()`] and one of the rotation methods, in order to move them
    /// apart from each other.
    #[inline]
    pub fn from_lengths_and_radii(
        lengths: impl IntoIterator<Item = impl Into<crate::components::Length>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(lengths, radii)
    }
}
