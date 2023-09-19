use crate::datatypes::UVec3D;

use super::MeshProperties;

impl MeshProperties {
    #[inline]
    pub fn from_triangle_indices(indices: impl IntoIterator<Item = impl Into<UVec3D>>) -> Self {
        Self(crate::datatypes::MeshProperties::from_triangle_indices(
            indices,
        ))
    }
}
