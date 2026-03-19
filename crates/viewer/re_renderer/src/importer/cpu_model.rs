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
struct CpuMeshInstance {
    mesh: CpuModelMeshKey,
    world_from_mesh: glam::Affine3A,
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
    meshes: SlotMap<CpuModelMeshKey, CpuMesh>,
    instances: Vec<CpuMeshInstance>,
    bbox: macaw::BoundingBox,
}

impl CpuModel {
    /// Creates a new [`CpuModel`] from a single [`CpuMesh`], creating a single instance with identity transform.
    pub fn from_single_mesh(mesh: CpuMesh) -> Self {
        let mut model = Self::default();
        let key = model.add_mesh(mesh);
        model.add_instance(key, glam::Affine3A::IDENTITY);
        model
    }

    /// Adds a [`CpuMesh`] to the model and returns its key.
    ///
    /// The mesh is not instantiated until [`Self::add_instance`] is called with the returned key.
    pub fn add_mesh(&mut self, mesh: CpuMesh) -> CpuModelMeshKey {
        self.meshes.insert(mesh)
    }

    /// Adds an instance of a mesh with the given transform, updating the model's bounding box.
    pub fn add_instance(&mut self, mesh_key: CpuModelMeshKey, world_from_mesh: glam::Affine3A) {
        if let Some(mesh) = self.meshes.get(mesh_key) {
            self.bbox = self
                .bbox
                .union(mesh.bbox.transform_affine3(&world_from_mesh));
        }
        self.instances.push(CpuMeshInstance {
            mesh: mesh_key,
            world_from_mesh,
        });
    }

    /// The bounding box of the model, accumulated from all instances and their transforms.
    pub fn bbox(&self) -> macaw::BoundingBox {
        self.bbox
    }

    /// Overrides the albedo factor on all materials of all meshes in the model.
    pub fn override_albedo_factor(&mut self, albedo_factor: crate::Rgba) {
        for (_key, mesh) in &mut self.meshes {
            for material in &mut mesh.materials {
                material.albedo_factor = albedo_factor;
            }
        }
    }

    /// Converts the entire model into a series of mesh instances that can be rendered.
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
                    cull_mode: Default::default(),
                })
            })
            .collect())
    }
}
