use slotmap::SlotMap;

use crate::{
    mesh::{CpuMesh, MeshError},
    renderer::MeshInstance,
};

slotmap::new_key_type! {
    /// Key for identifying a cpu mesh in a model.
    pub struct CpuModelMeshKey;
}

/// Like [`MeshInstance`], but for CPU sided usage in a [`CpuModel`] only.
pub struct CpuMeshInstance {
    pub mesh: CpuModelMeshKey,
    pub world_from_mesh: glam::Affine3A,
    // TODO(andreas): Expose other properties we have on [`MeshInstance`].
}

/// A model as stored on the CPU.
///
/// This is the output of a model loader and is ready to be converted into
/// a series of [`MeshInstance`]s that can be rendered.
///
/// This is meant as a useful intermediate structure for doing post-processing steps on the model prior to gpu upload.
#[derive(Default)]
pub struct CpuModel {
    pub meshes: SlotMap<CpuModelMeshKey, CpuMesh>,
    pub instances: Vec<CpuMeshInstance>,
}

impl CpuModel {
    /// Creates a new [`CpuModel`] from a single [`CpuMesh`], creating a single instance with identity transform.
    pub fn from_single_mesh(mesh: CpuMesh) -> Self {
        let mut model = Self::default();
        model.add_single_instance_mesh(mesh);
        model
    }

    /// Adds a new [`CpuMesh`] to the model, creating a single instance with identity transform.
    pub fn add_single_instance_mesh(&mut self, mesh: CpuMesh) {
        let mesh_key = self.meshes.insert(mesh);
        self.instances.push(CpuMeshInstance {
            mesh: mesh_key,
            world_from_mesh: glam::Affine3A::IDENTITY,
        });
    }

    /// Converts the entire model into a serious of mesh instances that can be rendered.
    // TODO(andreas): Should offer the option to discard or not discard the original mesh information
    // since this can be a significant memory overhead.
    pub fn to_gpu() -> Result<Vec<MeshInstance>, MeshError> {
        todo!()
    }
}
