#[cfg(feature = "import-obj")]
pub mod obj;

#[cfg(feature = "import-gltf")]
pub mod gltf;

use macaw::Vec3Ext as _;

use crate::{renderer::MeshInstance, resource_managers::MeshManager};

pub fn to_uniform_scale(scale: glam::Vec3) -> f32 {
    if scale.has_equal_components(0.00001) {
        scale.x
    } else {
        let uniform_scale = (scale.x * scale.y * scale.z).cbrt();
        re_log::warn!("mesh has non-uniform scale ({:?}). This is currently not supported. Using geometric mean {}", scale,uniform_scale);
        uniform_scale
    }
}

pub fn calculate_bounding_box(
    mesh_manager: &MeshManager,
    instances: &[MeshInstance],
) -> macaw::BoundingBox {
    macaw::BoundingBox::from_points(instances.iter().flat_map(|i| {
        let mesh = mesh_manager.get(i.mesh).unwrap();
        mesh.vertex_positions
            .iter()
            .map(|p| i.world_from_mesh.transform_point3(*p))
    }))
}
