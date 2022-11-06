use macaw::Vec3Ext;

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
