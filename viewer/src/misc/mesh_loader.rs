use anyhow::{anyhow, Context as _};
use log_types::{EncodedMesh3D, Mesh3D, MeshFormat, RawMesh3D};
use three_d::*;

pub struct CpuMesh {
    name: String,
    meshes: Vec<three_d::CpuMesh>,
    materials: Vec<three_d::CpuMaterial>,
    bbox: macaw::BoundingBox,
}

pub struct GpuMesh {
    pub name: String,
    pub models: Vec<InstancedModel<PhysicalMaterial>>,
    // pub materials: Vec<PhysicalMaterial>,
}

impl CpuMesh {
    pub fn load(name: String, mesh: &Mesh3D) -> anyhow::Result<Self> {
        // TODO: load CpuMesh in background thread.
        match mesh {
            Mesh3D::Encoded(encoded_mesh) => Self::load_encoded_mesh(name, encoded_mesh),
            Mesh3D::Raw(raw_mesh) => Self::load_raw_mesh(name, raw_mesh),
        }
    }

    pub fn load_raw(name: String, format: MeshFormat, bytes: &[u8]) -> anyhow::Result<Self> {
        crate::profile_function!();
        let path = "mesh";
        let mut loaded = three_d::io::Loaded::new();
        loaded.insert_bytes(path, bytes.to_vec());

        let (mut meshes, materials) = match format {
            MeshFormat::Glb | MeshFormat::Gltf => loaded.gltf(path),
            MeshFormat::Obj => loaded.obj(path),
        }
        .map_err(to_anyhow)
        .with_context(|| format!("loading {format:?}"))?;

        for mesh in &mut meshes {
            if mesh.tangents.is_none() {
                mesh.compute_tangents().ok();
            }
        }

        let bbox = bbox(&meshes);

        Ok(Self {
            name,
            meshes,
            materials,
            bbox,
        })
    }

    fn load_encoded_mesh(name: String, encoded_mesh: &EncodedMesh3D) -> anyhow::Result<Self> {
        crate::profile_function!();
        let EncodedMesh3D {
            format,
            bytes,
            transform,
        } = encoded_mesh;

        let mut slf = Self::load_raw(name, *format, bytes)?;

        let [c0, c1, c2, c3] = *transform;
        let root_transform = three_d::Mat4::from_cols(c0.into(), c1.into(), c2.into(), c3.into());
        for mesh in &mut slf.meshes {
            mesh.transform(&root_transform)
                .map_err(to_anyhow)
                .context("Bad object transform")?;
        }

        Ok(slf)
    }

    fn load_raw_mesh(name: String, raw_mesh: &RawMesh3D) -> anyhow::Result<Self> {
        crate::profile_function!();
        let RawMesh3D { positions, indices } = raw_mesh;
        let positions = positions
            .iter()
            .map(|&[x, y, z]| three_d::vec3(x, y, z))
            .collect();

        let material_name = "material_name".to_string(); // whatever

        let mut mesh = three_d::CpuMesh {
            name: name.clone(),
            positions: three_d::Positions::F32(positions),
            indices: Some(three_d::Indices::U32(
                indices.iter().flat_map(|triangle| *triangle).collect(),
            )),
            material_name: Some(material_name.clone()),
            ..Default::default()
        };
        mesh.compute_normals();

        let meshes = vec![mesh];
        let bbox = bbox(&meshes);

        let material = three_d::CpuMaterial {
            name: material_name,
            ..Default::default()
        };

        Ok(Self {
            name,
            meshes,
            materials: vec![material],
            bbox,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bbox(&self) -> &macaw::BoundingBox {
        &self.bbox
    }

    pub fn to_gpu(&self, three_d: &three_d::Context) -> anyhow::Result<GpuMesh> {
        crate::profile_function!();

        let mut materials = Vec::new();
        for m in self.materials.iter() {
            materials.push(PhysicalMaterial::new(three_d, m).map_err(to_anyhow)?);
        }

        let mut models = Vec::new();
        for m in self.meshes.iter() {
            let material = materials
                .iter()
                .find(|material| Some(&material.name) == m.material_name.as_ref())
                .context("missing material")?
                .clone();

            let m = InstancedModel::new_with_material(three_d, &Default::default(), m, material)
                .map_err(to_anyhow)?;
            models.push(m);
        }

        Ok(GpuMesh {
            name: self.name.clone(),
            models,
            // materials,
        })
    }
}

#[allow(clippy::needless_pass_by_value)]
fn to_anyhow(err: Box<dyn std::error::Error>) -> anyhow::Error {
    anyhow!("{}", err)
}

fn bbox(meshes: &[three_d::CpuMesh]) -> macaw::BoundingBox {
    let mut bbox = macaw::BoundingBox::nothing();
    for mesh in meshes {
        match &mesh.positions {
            three_d::Positions::F32(positions) => {
                for pos in positions {
                    bbox.extend(glam::Vec3::from(pos.as_array()));
                }
            }
            three_d::Positions::F64(positions) => {
                for pos in positions {
                    bbox.extend(glam::Vec3::from(pos.cast::<f32>().unwrap().as_array()));
                }
            }
        }
    }
    bbox
}
