use super::Triangles3D;

impl Triangles3D {
    /// Creates a new [`Triangles3D`] from vertex positions.
    ///
    /// Every consecutive triplet of positions forms one triangle.
    #[inline]
    pub fn from_vertices(
        vertex_positions: impl IntoIterator<Item = impl Into<crate::components::Position3D>>,
    ) -> Self {
        Self::new(vertex_positions)
    }
}
