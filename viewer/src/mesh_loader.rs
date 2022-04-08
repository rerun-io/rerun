use anyhow::{anyhow, Context as _};
use log_types::{Mesh3D, MeshFormat};
use three_d::*;

pub fn load(three_d: &three_d::Context, mesh_data: &Mesh3D) -> anyhow::Result<GpuMesh> {
    // TODO: load CpuMesh in background thread.
    CpuMesh::load(mesh_data)?.to_gpu(three_d)
}

struct CpuMesh {
    meshes: Vec<three_d::CpuMesh>,
    materials: Vec<three_d::CpuMaterial>,
}

pub struct GpuMesh {
    pub models: Vec<Model<PhysicalMaterial>>,
    // pub materials: Vec<PhysicalMaterial>,
    pub aabb: AxisAlignedBoundingBox,
}

impl CpuMesh {
    fn load(mesh_data: &Mesh3D) -> anyhow::Result<Self> {
        let path = "mesh";
        let mut loaded = three_d::io::Loaded::new();
        loaded.insert_bytes(path, mesh_data.bytes.to_vec());

        let (mut meshes, materials) = match mesh_data.format {
            MeshFormat::Glb | MeshFormat::Gltf => loaded.gltf(path),
            MeshFormat::Obj => loaded.obj(path),
        }
        .map_err(to_anyhow)
        .context("loading gltf")?;

        let [c0, c1, c2, c3] = mesh_data.transform;
        let root_transform = three_d::Mat4::from_cols(c0.into(), c1.into(), c2.into(), c3.into());
        for mesh in &mut meshes {
            mesh.transform(&root_transform);
            if mesh.tangents.is_none() {
                mesh.compute_tangents().ok();
            }
        }

        Ok(Self { meshes, materials })
    }

    fn to_gpu(&self, three_d: &three_d::Context) -> anyhow::Result<GpuMesh> {
        let mut materials = Vec::new();
        for m in self.materials.iter() {
            materials.push(PhysicalMaterial::new(three_d, m).map_err(to_anyhow)?);
        }

        let mut models = Vec::new();
        let mut aabb = AxisAlignedBoundingBox::EMPTY;
        for m in self.meshes.iter() {
            let material = materials
                .iter()
                .find(|material| Some(&material.name) == m.material_name.as_ref())
                .context("missing material")?
                .clone();

            let m = Model::new_with_material(three_d, m, material).map_err(to_anyhow)?;
            aabb.expand_with_aabb(&m.aabb());
            models.push(m);
        }

        Ok(GpuMesh {
            models,
            // materials,
            aabb,
        })
    }
}

#[allow(clippy::needless_pass_by_value)]
fn to_anyhow(err: Box<dyn std::error::Error>) -> anyhow::Error {
    anyhow!("{}", err)
}
