use crate::datatypes::UVec3D;

use super::MeshProperties;

impl MeshProperties {
    /// Create a new [`MeshProperties`] from an iterable of the triangle indices.
    ///
    /// Each triangle is defined by a [`UVec3D`] (or something that can be converted into it),
    /// where the 3 values are the indices of the 3 vertices of the triangle.
    #[inline]
    pub fn from_triangle_indices(indices: impl IntoIterator<Item = impl Into<UVec3D>>) -> Self {
        Self(crate::datatypes::MeshProperties::from_triangle_indices(
            indices,
        ))
    }
}
