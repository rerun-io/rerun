use anyhow::Context as _;
use re_log_types::{EncodedMesh3D, Mesh3D, MeshFormat, RawMesh3D};
use three_d::*;

pub struct CpuMesh {
    name: String,
    meshes: Vec<three_d::CpuMesh>,
    materials: Vec<three_d::CpuMaterial>,
    bbox: macaw::BoundingBox,
}

pub struct GpuMesh {
    pub name: String,
    pub meshes: Vec<Gm<InstancedMesh, PhysicalMaterial>>,
    // pub materials: Vec<PhysicalMaterial>,
}

impl CpuMesh {
    pub fn load(name: String, mesh: &Mesh3D) -> anyhow::Result<Self> {
        // TODO(emilk): load CpuMesh in background thread.
        match mesh {
            Mesh3D::Encoded(encoded_mesh) => Self::load_encoded_mesh(name, encoded_mesh),
            Mesh3D::Raw(raw_mesh) => Ok(Self::load_raw_mesh(name, raw_mesh)),
        }
    }

    pub fn load_raw(name: String, format: MeshFormat, bytes: &[u8]) -> anyhow::Result<Self> {
        crate::profile_function!();

        let path = match format {
            MeshFormat::Glb => "mesh.glb",
            MeshFormat::Gltf => "mesh.gltf",
            MeshFormat::Obj => "mesh.obj",
        };

        let mut loaded = three_d_asset::io::RawAssets::new();
        loaded.insert(path, bytes.to_vec());

        let three_d::CpuModel {
            geometries: mut meshes,
            materials,
        } = loaded
            .deserialize(path)
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
        let root_transform = three_d::Mat4::from_cols(
            three_d::Vec3::from(c0).extend(0.0),
            three_d::Vec3::from(c1).extend(0.0),
            three_d::Vec3::from(c2).extend(0.0),
            three_d::Vec3::from(c3).extend(1.0),
        );
        for mesh in &mut slf.meshes {
            mesh.transform(&root_transform)
                .context("Bad object transform")?;
        }

        Ok(slf)
    }

    fn load_raw_mesh(name: String, raw_mesh: &RawMesh3D) -> Self {
        crate::profile_function!();
        let RawMesh3D { positions, indices } = raw_mesh;
        let positions = positions
            .iter()
            .map(|&[x, y, z]| three_d::vec3(x, y, z))
            .collect();

        let material_name = "material_name".to_owned(); // whatever

        let mut mesh = three_d::CpuMesh {
            name: name.clone(),
            positions: three_d_asset::Positions::F32(positions),
            indices: Some(three_d_asset::Indices::U32(
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

        Self {
            name,
            meshes,
            materials: vec![material],
            bbox,
        }
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
        for m in &self.materials {
            materials.push(PhysicalMaterial::new(three_d, m));
        }

        let mut meshes = Vec::new();
        for mesh in &self.meshes {
            let material = materials
                .iter()
                .find(|material| Some(&material.name) == mesh.material_name.as_ref())
                .context("missing material")?
                .clone();

            let gm = Gm::new(
                InstancedMesh::new(three_d, &Default::default(), mesh),
                material,
            );
            meshes.push(gm);
        }

        Ok(GpuMesh {
            name: self.name.clone(),
            meshes,
            // materials,
        })
    }
}

fn bbox(meshes: &[three_d::CpuMesh]) -> macaw::BoundingBox {
    let mut bbox = macaw::BoundingBox::nothing();
    for mesh in meshes {
        match &mesh.positions {
            three_d::Positions::F32(positions) => {
                for pos in positions {
                    bbox.extend(glam::vec3(pos.x, pos.y, pos.z));
                }
            }
            three_d::Positions::F64(positions) => {
                for pos in positions {
                    let pos = pos.cast::<f32>().unwrap();
                    bbox.extend(glam::vec3(pos.x, pos.y, pos.z));
                }
            }
        }
    }
    bbox
}
