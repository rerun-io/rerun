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
