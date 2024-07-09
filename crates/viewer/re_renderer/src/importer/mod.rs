#[cfg(feature = "import-obj")]
pub mod obj;

#[cfg(feature = "import-gltf")]
pub mod gltf;

#[cfg(feature = "import-stl")]
pub mod stl;

use macaw::Vec3Ext as _;

use crate::renderer::MeshInstance;

pub fn to_uniform_scale(scale: glam::Vec3) -> f32 {
    if scale.has_equal_components(0.001) {
        scale.x
    } else {
        let uniform_scale = (scale.x * scale.y * scale.z).cbrt();
        re_log::warn!("mesh has non-uniform scale ({:?}). This is currently not supported. Using geometric mean {}", scale,uniform_scale);
        uniform_scale
    }
}

pub fn calculate_bounding_box(instances: &[MeshInstance]) -> macaw::BoundingBox {
    macaw::BoundingBox::from_points(
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
