use slotmap::SlotMap;

use crate::{
    mesh::{CpuMesh, MeshError},
    renderer::MeshInstance,
};

slotmap::new_key_type! {
    /// Key for identifying a cpu mesh in a model.
    pub struct CpuMeshKey;
}

/// Like [`MeshInstance`], but for CPU sided usage in a [`CpuModel`] only.
pub struct CpuInstance {
    pub mesh: CpuMeshKey,
    pub transform: glam::Affine3A,
}

/// A model as stored on the CPU.
///
/// This is the output of a model loader and is ready to be converted into
/// a series of [`MeshInstance`]s that can be rendered.
pub struct CpuModel {
    pub meshes: SlotMap<CpuMeshKey, CpuMesh>,
    pub instances: Vec<CpuInstance>,
}

impl CpuModel {
    /// Converts the entire model into a serious of mesh instances that can be rendered.
    fn to_gpu() -> Result<Vec<MeshInstance>, MeshError> {
        todo!()
    }
}
