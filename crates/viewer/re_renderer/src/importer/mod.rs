#[cfg(feature = "import-obj")]
pub mod obj;

#[cfg(feature = "import-gltf")]
pub mod gltf;

#[cfg(feature = "import-stl")]
pub mod stl;

use crate::renderer::MeshInstance;

// TODO: make this part of the cpu model only.
pub fn calculate_bounding_box(instances: &[MeshInstance]) -> re_math::BoundingBox {
    re_math::BoundingBox::from_points(
        instances
            .iter()
            .filter_map(|mesh_instance| {
                mesh_instance.mesh.as_ref().map(|mesh| {
                    mesh.vertex_positions
                        .iter()
                        .map(|p| mesh_instance.world_from_mesh.transform_point3(*p))
                })
            })
            .flatten(),
    )
}
