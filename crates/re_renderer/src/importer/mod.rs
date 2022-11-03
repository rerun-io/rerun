#[cfg(feature = "import-obj")]
pub mod obj;

#[cfg(feature = "import-gltf")]
pub mod gltf;

#[derive(Default)]
pub struct ModelImportData {
    pub meshes: Vec<crate::mesh::MeshData>,
    pub instances: Vec<ImportMeshInstance>,
}

// TODO(andreas) better formalize this - what is the exact relation to MeshRenderer instance
pub struct ImportMeshInstance {
    pub mesh_idx: usize,
    pub transform: macaw::Conformal3,
}

impl ModelImportData {
    pub fn calculate_bounding_box(&self) -> macaw::BoundingBox {
        macaw::BoundingBox::from_points(self.instances.iter().flat_map(|instance| {
            self.meshes[instance.mesh_idx]
                .vertex_positions
                .iter()
                .map(|p| instance.transform.transform_point3(*p))
        }))
    }
}
