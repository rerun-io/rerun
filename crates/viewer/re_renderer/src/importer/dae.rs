//! Collada (.dae) scene loader for Rerun
//!
//! ⚠️ The Collada spec is extremely broad, and this implementation only supports a small subset of it:
//!   - `<triangles>` primitives, all other primitives are ignored.
//!   - Transform stack made of `<matrix>`, `<translate>`, `<rotate>` and
//!     `<scale>`; unsupported transform tags are skipped with a warning.
//!   - One set of positions, normals and optional tex‑coords per vertex.
//!   - Material diffuse colors from Blinn, Phong, Lambert, and Constant shaders.
//!
//! ⚠️ Texture support is not yet implemented. Only diffuse colors are loaded.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

use ahash::HashMap;
use regex::{Captures, Regex};
use smallvec::smallvec;
use thiserror::Error;

use crate::{
    CpuMeshInstance, CpuModel, CpuModelMeshKey, DebugLabel, RenderContext, Rgba32Unmul,
    mesh::{self, CpuMesh},
};

use dae_parser::{
    Document, Effect, Geometry, Instance, Material as DaeMaterial, Node as DaeNode, Shader,
    Transform as DaeTransform, VisualScene,
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

    let buffer = sanitize_dae_ids(buffer);

    load_dae_from_buffer_inner(buffer.as_ref(), ctx)
}


fn sanitize_dae_ids(buffer: &[u8]) -> Cow<'_, [u8]> {
    if !buffer.windows(3).any(|w| w == b"id=") {
        return Cow::Borrowed(buffer);
    }

    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"id=(["'])([^"']+)["']"#).unwrap());

    let content = String::from_utf8_lossy(buffer);
    let mut seen = HashSet::new();
    let mut modified = false;

    let new_content = re.replace_all(&content, |caps: &Captures<'_>| {
        let quote = caps[1].to_string();
        let id = caps[2].to_string();

        if seen.insert(id.clone()) {
            caps[0].to_string()
        } else {
            modified = true;
            let new_id = format!("{}_dup", id);
            re_log::warn_once!(
                "Renamed duplicate ID in DAE file to prevent parser panic: '{}' -> '{}'",
                id,
                new_id
            );
            format!("id={}{}{}", quote, new_id, quote)
        }
    });

    if modified {
        Cow::Owned(new_content.into_owned().into_bytes())
    } else {
        Cow::Borrowed(buffer)
    }
}

fn load_dae_from_buffer_inner(
    buffer: &[u8],
    ctx: &RenderContext,
) -> Result<CpuModel, DaeImportError> {
    let document = Document::from_reader(buffer).map_err(DaeImportError::Parser)?;
    let maps = document.local_maps();

    // TODO(#12335): Respect up_axis from DAE file via ViewCoordinates.

    // Check for textures and warn if found
    check_for_textures(&document);

    let mut model = CpuModel::default();
    let mut mesh_keys: HashMap<String, CpuModelMeshKey> = HashMap::default();

    for geometry in document.iter::<Geometry>() {
        // Only meshes -> triangles
        let Some(mesh_element) = geometry.element.as_mesh() else {
            re_log::debug_once!("Skipping non-mesh geometry element (e.g., camera or light)");
            continue;
        };

        // Skip geometries that *do not* contain a <triangles> primitive.
        let Some(triangles) = mesh_element.elements.iter().find_map(|p| p.as_triangles()) else {
            re_log::debug_once!(
                "Skipping geometry without <triangles> primitive (only <triangles> are supported)"
            );
            continue;
        };

        let cpu_mesh = import_geometry(geometry, mesh_element, triangles, &maps, ctx)?;
        let key = model.meshes.insert(cpu_mesh);
        let geom_id = geometry
            .id
            .as_deref()
            .or(geometry.name.as_deref())
            .unwrap_or("<unnamed geometry>")
            .to_owned();
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
        .map_err(|_err| DaeImportError::NoTriangles)?;
    let dae_importer: DaeImporter<'_> = triangles
        .importer(maps, vertex_importer)
        .map_err(|_err| DaeImportError::NoTriangles)?;

    let prim_data = triangles
        .data
        .as_deref()
        .ok_or(DaeImportError::NoTriangles)?;

    let mut pos_raw = Vec::new();
    let mut normals = Vec::new();
    let mut texcoords = Vec::new();
    let mut tri_indices = Vec::<glam::UVec3>::new();

    for (i, v) in dae_importer.read::<(), Vertex>(&(), prim_data).enumerate() {
        pos_raw.push(v.position);
        normals.push(v.normal);
        texcoords.push(v.texcoord);

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
            .unwrap_or_default(),
    );

    // Extract material color from the triangles' material reference
    let albedo_factor = triangles
        .material
        .as_ref()
        .and_then(|mat_symbol| extract_material_color(mat_symbol, maps))
        .unwrap_or(crate::Rgba::WHITE);

    let material = mesh::Material {
        label: label.clone(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor,
    };

    let vertex_positions = bytemuck::cast_vec(pos_raw);
    let bbox = macaw::BoundingBox::from_points(vertex_positions.iter().copied());

    let cpu_mesh = mesh::CpuMesh {
        label: label.clone(),
        triangle_indices: tri_indices,
        vertex_positions,
        vertex_normals: bytemuck::cast_vec(normals),
        vertex_colors: vec![Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: bytemuck::cast_vec(texcoords),
        materials: smallvec![material],
        bbox,
    };

    cpu_mesh.sanity_check()?;
    Ok(cpu_mesh)
}

/// Check if the DAE document contains textures and emit a warning if so.
fn check_for_textures(document: &Document) {
    use dae_parser::Image;

    let has_images = document.iter::<Image>().next().is_some();
    if has_images {
        re_log::warn_once!(
            "DAE file contains texture images, but texture support is not yet implemented. Only diffuse colors will be loaded."
        );
    }
}

/// Extract the diffuse color from a material symbol by looking it up in the document.
fn extract_material_color(
    material_symbol: &str,
    maps: &dae_parser::LocalMaps<'_>,
) -> Option<crate::Rgba> {
    // we obtain the diffuse color by first looking up the material,
    let material = maps.get_str::<DaeMaterial>(material_symbol)?;

    // then the effect it references
    let effect_url = &material.instance_effect.url.val;
    let effect_id = match effect_url {
        dae_parser::Url::Fragment(frag) => frag.trim_start_matches('#'),
        dae_parser::Url::Other(_) => return None,
    };
    let effect = maps.get_str::<Effect>(effect_id)?;
    let profile_common = effect.get_common_profile()?;

    // and finally the shader inside the effect.
    let shader = profile_common.technique.data.shaders.first()?;

    let diffuse_color = match shader {
        Shader::Blinn(blinn) => blinn.diffuse.as_ref()?.as_color(),
        Shader::Phong(phong) => phong.diffuse.as_ref()?.as_color(),
        Shader::Lambert(lambert) => lambert.diffuse.as_ref()?.as_color(),
        Shader::Constant(constant) => constant.emission.as_ref()?.as_color(),
    }?;

    // This is not a hard-coded color.
    #[expect(clippy::disallowed_methods)]
    Some(crate::Rgba::from_rgba_unmultiplied(
        diffuse_color[0],
        diffuse_color[1],
        diffuse_color[2],
        diffuse_color[3],
    ))
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
                // COLLADA matrices are written in row-major order in XML
                local_mat *= Mat4::from_cols_array(&matrix.0).transpose();
            }
            DaeTransform::Translate(translation) => {
                local_mat *= Mat4::from_translation(Vec3::from_array(*translation.0));
            }
            DaeTransform::Scale(scale) => {
                local_mat *= Mat4::from_scale(Vec3::from_array(*scale.0));
            }
            DaeTransform::Rotate(rotation) => {
                let axis = Vec3::from_slice(&rotation.0[0..3]);
                let angle = rotation.0[3];

                local_mat *= Mat4::from_quat(Quat::from_axis_angle(axis, angle.to_radians()));
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
            dae_parser::Url::Fragment(frag) => frag.trim_start_matches('#').to_owned(),
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
    texcoord: [f32; 2],
}

impl<'a> VertexLoad<'a> for Vertex {
    fn position(_: &(), reader: &SourceReader<'a, XYZ>, i: u32) -> Self {
        Self {
            position: reader.get(i as usize),
            normal: [0.0; 3],
            texcoord: [0.0; 2],
        }
    }

    fn add_normal(&mut self, _: &(), reader: &SourceReader<'a, XYZ>, i: u32) {
        self.normal = reader.get(i as usize);
    }

    fn add_texcoord(&mut self, _: &(), r: &SourceReader<'a, ST>, i: u32, _set: Option<u32>) {
        self.texcoord = r.get(i as usize);
    }
}
