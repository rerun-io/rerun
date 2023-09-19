use crate::datatypes::UVec3D;

use super::MeshProperties;

impl MeshProperties {
    #[inline]
    pub fn from_triangle_indices(indices: impl IntoIterator<Item = impl Into<UVec3D>>) -> Self {
        Self {
            triangle_indices: Some(
                indices
                    .into_iter()
                    .map(Into::into)
                    .flat_map(|xyz| [xyz.x(), xyz.y(), xyz.z()])
                    .collect(),
            ),
        }
    }
}
