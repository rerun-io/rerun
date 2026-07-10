use super::Cones3D;

impl Cones3D {
    /// Creates a new [`Cones3D`] with the given axis-aligned lengths and base radii.
    ///
    /// For multiple cones, you should generally follow this with
    /// [`Cones3D::with_centers()`] and one of the rotation methods, in order to move them
    /// apart from each other.
    #[inline]
    pub fn from_lengths_and_radii(
        lengths: impl IntoIterator<Item = impl Into<crate::components::Length>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(lengths, radii)
    }
}
