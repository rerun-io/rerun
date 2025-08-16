//! Collada (.dae) scene loader for Rerun
//!
//! ⚠️ The Collada spec is extremely broad, and this implementation only supports a small subset of it:
//!   * `<triangles>` primitives, all other primitives are ignored.
//!   * Transform stack made of `<matrix>`, `<translate>`, `<rotate>` and
//!     `<scale>`; unsupported transform tags are skipped with a warning.
//!   * One set of positions, normals and optional tex‑coords per vertex.

use ahash::HashMap;
use smallvec::smallvec;
use thiserror::Error;

use crate::{
    CpuMeshInstance, CpuModel, CpuModelMeshKey, DebugLabel, RenderContext, Rgba32Unmul,
    mesh::{self, CpuMesh},
};

use dae_parser::{
    Document, Geometry, Instance, Node as DaeNode, Transform as DaeTransform, VisualScene,
    geom::{Importer as DaeImporter, VertexImporter, VertexLoad},
    source::{ST, SourceReader, XYZ},
};

#[derive(Error, Debug)]
pub enum DaeImportError {
    #[error("collada parse error: {0:?}")]
    Parser(dae_parser::Error),

    #[error("no `<visual_scene>` element found")]
    NoVisualScene,

    #[error("geometry with `<triangles>` not found")]
    NoTriangles,

    #[error("mesh import error: {0}")]
    Mesh(#[from] mesh::MeshError),
}

pub fn load_dae_from_buffer(
    buffer: &[u8],
    ctx: &RenderContext,
) -> Result<CpuModel, DaeImportError> {
    re_tracing::profile_function!();

    let document = Document::from_reader(buffer).map_err(DaeImportError::Parser)?;
    let maps = document.local_maps();

    let mut model = CpuModel::default();
    let mut mesh_keys: HashMap<String, CpuModelMeshKey> = HashMap::default();

    for geometry in document.iter::<Geometry>() {
        // Only meshes -> triangles
        let mesh_element = match geometry.element.as_mesh() {
            Some(m) => m,
            None => continue,
        };

        // Skip geometries that *do not* contain a <triangles> primitive.
        let triangles = match mesh_element.elements.iter().find_map(|p| p.as_triangles()) {
            Some(t) => t,
            None => continue,
        };

        let cpu_mesh = import_geometry(geometry, mesh_element, triangles, &maps, ctx)?;
        let key = model.meshes.insert(cpu_mesh);
        let geom_id = geometry
            .id
            .as_deref()
            .or_else(|| geometry.name.as_deref())
            .unwrap_or("<unnamed geometry>")
            .to_string();
        mesh_keys.insert(geom_id, key);
    }

    let mut instances = Vec::new();

    let mut any_scene = false;
    for scene in document.iter::<VisualScene>() {
        any_scene = true;
        for root in &scene.nodes {
            gather_instances_recursive(&mut instances, root, &glam::Affine3A::IDENTITY, &mesh_keys);
        }
    }

    if !any_scene {
        return Err(DaeImportError::NoVisualScene);
    }

    model.instances = instances;
    Ok(model)
}

fn import_geometry(
    geo: &Geometry,
    mesh: &dae_parser::Mesh,
    triangles: &dae_parser::Triangles,
    maps: &dae_parser::LocalMaps<'_>,
    ctx: &RenderContext,
) -> Result<CpuMesh, DaeImportError> {
    let vertices = mesh.vertices.as_ref().ok_or(DaeImportError::NoTriangles)?;
    let vertex_importer: VertexImporter<'_> = vertices
        .importer(maps)
        .map_err(|_| DaeImportError::NoTriangles)?;
    let dae_importer: DaeImporter<'_> = triangles
        .importer(maps, vertex_importer)
        .map_err(|_| DaeImportError::NoTriangles)?;

    let prim_data = triangles
        .data
        .as_deref()
        .ok_or(DaeImportError::NoTriangles)?;

    let mut pos_raw = Vec::new();
    let mut normals = Vec::new();
    let mut tri_indices = Vec::<glam::UVec3>::new();

    for (i, v) in dae_importer.read::<(), Vertex>(&(), prim_data).enumerate() {
        pos_raw.push(v.position);
        normals.push(v.normal);

        // Triangles are grouped in triplets
        if i % 3 == 2 {
            let base = i as u32 - 2;
            tri_indices.push(glam::UVec3::new(base, base + 1, base + 2));
        }
    }

    let num_vertices = pos_raw.len();
    let label = DebugLabel::from(
        geo.name
            .clone()
            .or_else(|| geo.id.clone())
            .unwrap_or_else(|| "".into()),
    );

    let material = mesh::Material {
        label: label.clone(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor: crate::Rgba::WHITE,
    };

    let cpu_mesh = mesh::CpuMesh {
        label: label.clone(),
        triangle_indices: tri_indices,
        vertex_positions: bytemuck::cast_vec(pos_raw),
        vertex_normals: bytemuck::cast_vec(normals),
        vertex_colors: vec![Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],
        materials: smallvec![material],
    };

    cpu_mesh.sanity_check()?;
    Ok(cpu_mesh)
}

fn gather_instances_recursive(
    out: &mut Vec<CpuMeshInstance>,
    node: &DaeNode,
    parent_tf: &glam::Affine3A,
    meshes: &HashMap<String, CpuModelMeshKey>,
) {
    use glam::{Affine3A, Mat4, Quat, Vec3};

    let mut local_mat = Mat4::IDENTITY;
    for t in &node.transforms {
        match t {
            DaeTransform::Matrix(matrix) => {
                local_mat = local_mat * Mat4::from_cols_array(&*matrix.0);
            }
            DaeTransform::Translate(translation) => {
                local_mat = local_mat * Mat4::from_translation(Vec3::from_array(*translation.0));
            }
            DaeTransform::Scale(scale) => {
                local_mat = local_mat * Mat4::from_scale(Vec3::from_array(*scale.0));
            }
            DaeTransform::Rotate(rotation) => {
                let axis = Vec3::from_slice(&rotation.0[0..3]);
                let angle = rotation.0[3];

                local_mat =
                    local_mat * Mat4::from_quat(Quat::from_axis_angle(axis, angle.to_radians()));
            }
            _ => {
                re_log::warn!("Ignoring unsupported Collada transform {t:?}");
            }
        }
    }

    let world_tf = *parent_tf * Affine3A::from_mat4(local_mat);

    for Instance::<Geometry> { url, .. } in &node.instance_geometry {
        let id = match url.val.clone() {
            // URI reference (e.g. "#Cube-mesh"), we need to strip the leading `#`.
            dae_parser::Url::Fragment(frag) => frag.trim_start_matches("#").to_owned(),
            dae_parser::Url::Other(other) => {
                // Non-fragment URL, we don't handle these
                re_log::warn_once!(
                    "<instance_geometry> with non-fragment URL {other} is not supported"
                );
                continue;
            }
        };

        if let Some(mesh_key) = meshes.get(&id) {
            out.push(CpuMeshInstance {
                mesh: *mesh_key,
                world_from_mesh: world_tf,
            });
        } else {
            re_log::warn_once!("<instance_geometry> references unknown geometry {id}");
        }
    }

    for child in &node.children {
        gather_instances_recursive(out, child, &world_tf, meshes);
    }
}

#[derive(Clone, Default)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl<'a> VertexLoad<'a> for Vertex {
    fn position(_: &(), reader: &SourceReader<'a, XYZ>, i: u32) -> Self {
        Self {
            position: reader.get(i as usize),
            normal: [0.0; 3],
        }
    }

    fn add_normal(&mut self, _: &(), reader: &SourceReader<'a, XYZ>, i: u32) {
        self.normal = reader.get(i as usize);
    }

    fn add_texcoord(&mut self, _: &(), _r: &SourceReader<'a, ST>, _i: u32, _set: Option<u32>) {
        // TODO(gijsd): add texture/material support
    }
}
