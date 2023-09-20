use crate::datatypes::UVec3D;

use super::MeshProperties;

impl MeshProperties {
    #[inline]
    pub fn from_triangle_indices(indices: impl IntoIterator<Item = impl Into<UVec3D>>) -> Self {
        Self {
            vertex_indices: Some(
                indices
                    .into_iter()
                    .map(Into::into)
                    .flat_map(|tri| [tri.x(), tri.y(), tri.z()])
                    .collect(),
            ),
        }
    }
}
