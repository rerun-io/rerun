use itertools::Itertools as _;
use macaw::Vec3Ext;

use crate::{renderer::MeshInstance, resource_managers::ResourceLifeTime, RenderContext};

#[cfg(feature = "import-obj")]
pub mod obj;

#[cfg(feature = "import-gltf")]
pub mod gltf;

#[derive(Default)]
pub struct ModelImportData {
    pub meshes: Vec<crate::mesh::Mesh>,
    pub instances: Vec<ImportMeshInstance>,
}

pub struct ImportMeshInstance {
    /// Index into [`ModelImportData::meshes`]
    pub mesh_idx: usize,
    /// Transforms the mesh into world coordinates.
    pub world_from_mesh: macaw::Conformal3,
}

impl ModelImportData {
    pub fn calculate_bounding_box(&self) -> macaw::BoundingBox {
        macaw::BoundingBox::from_points(self.instances.iter().flat_map(|instance| {
            self.meshes[instance.mesh_idx]
                .vertex_positions
                .iter()
                .map(|p| instance.world_from_mesh.transform_point3(*p))
        }))
    }

    /// Consumes the model import data and pushes all meshes to the mesh manager.
    ///
    /// Resolves all instance mesh indices to these new mesh handles and returns them.
    /// TODO(andreas) do we need a short-lived version of this? Unlikely as that frame-lived data typically doesn't arrive as mesh!
    pub fn push_to_mesh_manager(self, ctx: &mut RenderContext) -> Vec<MeshInstance> {
        let Self { meshes, instances } = self;

        let mesh_handles = meshes
            .into_iter()
            .map(|mesh| ctx.meshes.store_resource(mesh, ResourceLifeTime::LongLived))
            .collect_vec();
        instances
            .into_iter()
            .map(|import_instance| MeshInstance {
                mesh: mesh_handles[import_instance.mesh_idx],
                world_from_mesh: import_instance.world_from_mesh,
            })
            .collect()
    }
}

pub fn to_uniform_scale(scale: glam::Vec3) -> f32 {
    if scale.has_equal_components(0.00001) {
        scale.x
    } else {
        let uniform_scale = (scale.x * scale.y * scale.z).cbrt();
        re_log::warn!("mesh has non-uniform scale ({:?}). This is currently not supported. Using geometric mean {}", scale,uniform_scale);
        uniform_scale
    }
}
