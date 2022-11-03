#[cfg(feature = "glow")]
use anyhow::Context as _;
use re_log_types::{EncodedMesh3D, Mesh3D, MeshFormat, RawMesh3D};

pub struct CpuMesh {
    name: String,

    #[cfg(feature = "glow")]
    meshes: Vec<three_d::CpuMesh>,
    #[cfg(feature = "glow")]
    materials: Vec<three_d::CpuMaterial>,

    #[cfg(feature = "wgpu")]
    pub model_import: re_renderer::importer::ModelImportData,

    bbox: macaw::BoundingBox,
}

#[cfg(feature = "glow")]
pub struct GpuMesh {
    pub name: String,
    pub meshes: Vec<three_d::Gm<three_d::InstancedMesh, three_d::PhysicalMaterial>>,
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

        #[cfg(feature = "glow")]
        {
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
                #[cfg(feature = "wgpu")]
                model_import: Default::default(),
            })
        }
        #[cfg(not(feature = "glow"))]
        {
            let model_import = match format {
                MeshFormat::Glb => re_renderer::importer::gltf::load_gltf_from_buffer(bytes),
                MeshFormat::Gltf => re_renderer::importer::gltf::load_gltf_from_buffer(bytes),
                MeshFormat::Obj => re_renderer::importer::obj::load_obj_from_buffer(bytes),
            }?;
            let bbox = model_import.calculate_bounding_box();

            Ok(Self {
                name,
                bbox,
                #[cfg(feature = "wgpu")]
                model_import,
            })
        }
    }

    fn load_encoded_mesh(name: String, encoded_mesh: &EncodedMesh3D) -> anyhow::Result<Self> {
        crate::profile_function!();
        let EncodedMesh3D {
            format,
            bytes,
            transform,
        } = encoded_mesh;

        let mut slf = Self::load_raw(name, *format, bytes)?;
        #[cfg(feature = "glow")]
        {
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
        }
        #[cfg(not(feature = "glow"))]
        {
            let root_transform = macaw::Conformal3::from_affine3a_lossy(
                &glam::Affine3A::from_cols_array_2d(transform),
            );
            for instance in &mut slf.model_import.instances {
                instance.transform = instance.transform * root_transform;
            }
            slf.bbox = slf.model_import.calculate_bounding_box();
        }

        Ok(slf)
    }

    fn load_raw_mesh(name: String, raw_mesh: &RawMesh3D) -> Self {
        crate::profile_function!();
        #[cfg(feature = "glow")]
        let meshes = {
            let RawMesh3D { positions, indices } = raw_mesh;
            let positions = positions
                .iter()
                .map(|&[x, y, z]| three_d::vec3(x, y, z))
                .collect();

            let mut mesh = three_d::CpuMesh {
                name: name.clone(),
                positions: three_d_asset::Positions::F32(positions),
                indices: Some(three_d_asset::Indices::U32(
                    indices.iter().flat_map(|triangle| *triangle).collect(),
                )),
                material_name: Some("material_name".into()),
                ..Default::default()
            };
            mesh.compute_normals();
            vec![mesh]
        };
        #[cfg(feature = "glow")]
        let material = three_d::CpuMaterial {
            name: "material_name".to_owned(),
            ..Default::default()
        };

        let bbox = macaw::BoundingBox::from_points(
            raw_mesh.positions.iter().map(|p| glam::Vec3::from(*p)),
        );

        #[cfg(feature = "wgpu")]
        let label = name.clone().into();

        Self {
            name,
            #[cfg(feature = "glow")]
            meshes,
            #[cfg(feature = "glow")]
            materials: vec![material],
            bbox,

            #[cfg(feature = "wgpu")]
            model_import: re_renderer::importer::ModelImportData {
                instances: vec![re_renderer::importer::ImportMeshInstance {
                    mesh_idx: 0,
                    transform: macaw::Conformal3::IDENTITY,
                }],
                meshes: vec![re_renderer::mesh::MeshData {
                    label,
                    indices: raw_mesh.indices.iter().flatten().cloned().collect(),
                    vertex_positions: raw_mesh
                        .positions
                        .iter()
                        .map(|p| glam::Vec3::from(*p))
                        .collect(),
                    // TODO(andreas): Calculate normals
                    vertex_data: std::iter::repeat(
                        re_renderer::mesh::mesh_vertices::MeshVertexData {
                            normal: glam::Vec3::ZERO,
                            texcoord: glam::Vec2::ZERO,
                        },
                    )
                    .take(raw_mesh.positions.len())
                    .collect(),
                }],
            },
        }
    }

    /// Builds a cylinder mesh around the x-axis in the range [0..1] and with radius 1. The default material is used.
    #[cfg(feature = "glow")]
    pub(crate) fn cylinder(angle_subdivisions: u32) -> Self {
        let meshes = vec![three_d::CpuMesh::cylinder(angle_subdivisions)];
        let material = three_d::CpuMaterial {
            name: "cylinder_material".to_owned(),
            ..Default::default()
        };
        let bbox = bbox(&meshes);
        Self {
            name: "cylinder".to_owned(),
            meshes,
            materials: vec![material],
            bbox,
            #[cfg(feature = "wgpu")]
            model_import: Default::default(),
        }
    }

    /// Builds a cone mesh around the x-axis in the range [0..1] and with radius 1 at x=0. The default material is used.
    #[cfg(feature = "glow")]
    pub(crate) fn cone(angle_subdivisions: u32) -> Self {
        let meshes = vec![three_d::CpuMesh::cone(angle_subdivisions)];
        let material = three_d::CpuMaterial {
            name: "cone_material".to_owned(),
            ..Default::default()
        };

        let bbox = bbox(&meshes);
        Self {
            name: "cone".to_owned(),
            meshes,
            materials: vec![material],
            bbox,
            #[cfg(feature = "wgpu")]
            model_import: Default::default(),
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bbox(&self) -> &macaw::BoundingBox {
        &self.bbox
    }

    #[cfg(feature = "glow")]
    pub fn to_gpu(&self, three_d: &three_d::Context) -> GpuMesh {
        use three_d::*;
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
                .cloned()
                .unwrap_or_default();

            let gm = Gm::new(
                InstancedMesh::new(three_d, &Default::default(), mesh),
                material,
            );
            meshes.push(gm);
        }

        GpuMesh {
            name: self.name.clone(),
            meshes,
            // materials,
        }
    }
}

#[cfg(feature = "glow")]
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
