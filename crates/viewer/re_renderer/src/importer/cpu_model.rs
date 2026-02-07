use std::sync::Arc;

use slotmap::{SecondaryMap, SlotMap};

use crate::RenderContext;
use crate::mesh::{CpuMesh, GpuMesh, MeshError};
use crate::renderer::GpuMeshInstance;

slotmap::new_key_type! {
    /// Key for identifying a cpu mesh in a model.
    pub struct CpuModelMeshKey;
}

/// Like [`GpuMeshInstance`], but for CPU sided usage in a [`CpuModel`] only.
pub struct CpuMeshInstance {
    pub mesh: CpuModelMeshKey,
    pub world_from_mesh: glam::Affine3A,
    // TODO(andreas): Expose other properties we have on [`GpuMeshInstance`].
}

/// A collection of meshes & mesh instances on the CPU.
///
/// Note that there is currently no `GpuModel` equivalent, since
/// [`GpuMeshInstance`]es use shared ownership of [`GpuMesh`]es.
///
/// This is the output of a model loader and is ready to be converted into
/// a series of [`GpuMeshInstance`]s that can be rendered.
///
/// This is meant as a useful intermediate structure for doing post-processing steps on the model prior to gpu upload.
#[derive(Default)]
pub struct CpuModel {
    pub meshes: SlotMap<CpuModelMeshKey, CpuMesh>,
    pub instances: Vec<CpuMeshInstance>,
    pub bbox: macaw::BoundingBox,
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
        self.bbox = self.bbox.union(mesh.bbox);
        let mesh_key = self.meshes.insert(mesh);
        self.instances.push(CpuMeshInstance {
            mesh: mesh_key,
            world_from_mesh: glam::Affine3A::IDENTITY,
        });
    }

    /// Converts the entire model into a serious of mesh instances that can be rendered.
    ///
    /// Silently ignores:
    /// * instances with invalid mesh keys
    /// * unreferenced meshes
    pub fn into_gpu_meshes(self, ctx: &RenderContext) -> Result<Vec<GpuMeshInstance>, MeshError> {
        let mut gpu_meshes = SecondaryMap::with_capacity(self.meshes.len());
        for (mesh_key, mesh) in &self.meshes {
            gpu_meshes.insert(mesh_key, Arc::new(GpuMesh::new(ctx, mesh)?));
        }

        Ok(self
            .instances
            .into_iter()
            .filter_map(|instance| {
                Some(GpuMeshInstance {
                    gpu_mesh: gpu_meshes.get(instance.mesh)?.clone(),
                    world_from_mesh: instance.world_from_mesh,
                    additive_tint: Default::default(),
                    outline_mask_ids: Default::default(),
                    picking_layer_id: Default::default(),
                })
            })
            .collect())
    }
}
