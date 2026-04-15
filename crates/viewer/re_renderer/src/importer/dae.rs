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
use regex_lite::{Captures, Regex};
use smallvec::SmallVec;
use thiserror::Error;

use crate::{
    CpuModel, CpuModelMeshKey, Label, RenderContext, Rgba32Unmul,
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

    // Some DAE files, particularly those exported from CAD tools, contain duplicate id attributes in their
    // XML structure. The underlying `dae-parser` library panics when encountering these duplicates,
    // so we sanitize the XML before attempting to load it.
    let buffer = sanitize_dae_ids(buffer);

    load_dae_from_buffer_inner(buffer.as_ref(), ctx)
}

fn sanitize_dae_ids(buffer: &[u8]) -> Cow<'_, [u8]> {
    if !buffer.windows(3).any(|w| w == b"id=") {
        return Cow::Borrowed(buffer);
    }

    static RE: OnceLock<Regex> = OnceLock::new();
    // Note: we only want to match the global `id` here, not scoped `sid` attributes.
    let re = RE.get_or_init(|| Regex::new(r#"\bid=(["'])([^"']+)["']"#).unwrap());

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
            let new_id = format!("{id}_dup");
            re_log::warn_once!(
                "DAE file contains duplicate ID. Renaming it to avoid conflict: '{id}' -> '{new_id}'",
            );
            format!("id={quote}{new_id}{quote}")
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

    // Compute a correction matrix to rotate from the DAE file's coordinate system into Rerun's
    // default RFU (X=Right, Y=Forward, Z=Up) convention.
    let correction = up_axis_correction(document.asset.up_axis);

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

        // Collect all <triangles> primitives in this geometry.
        let all_triangles: Vec<_> = mesh_element
            .elements
            .iter()
            .filter_map(|p| p.as_triangles())
            .collect();

        if all_triangles.is_empty() {
            re_log::debug_once!(
                "Skipping geometry without <triangles> primitive (only <triangles> are supported)"
            );
            continue;
        }

        let cpu_mesh = import_geometry(geometry, mesh_element, &all_triangles, &maps, ctx)?;
        let key = model.add_mesh(cpu_mesh);
        let geom_id = geometry
            .id
            .as_deref()
            .or(geometry.name.as_deref())
            .unwrap_or("<unnamed geometry>")
            .to_owned();
        mesh_keys.insert(geom_id, key);
    }

    let mut any_scene = false;
    for scene in document.iter::<VisualScene>() {
        any_scene = true;
        // A <visual_scene> may have its own <asset><up_axis> that overrides the document level.
        let scene_correction = scene
            .asset
            .as_deref()
            .map_or(correction, |a| up_axis_correction(a.up_axis));
        for root in &scene.nodes {
            // Each root node uses its own <asset><up_axis> if present, otherwise the scene's.
            let root_correction = root
                .asset
                .as_deref()
                .map_or(scene_correction, |a| up_axis_correction(a.up_axis));
            gather_instances_recursive(
                &mut model,
                root,
                &glam::Affine3A::IDENTITY,
                // No correction has been applied to the parent (IDENTITY) yet.
                &glam::Affine3A::IDENTITY,
                &root_correction,
                &mesh_keys,
            );
        }
    }

    if !any_scene {
        return Err(DaeImportError::NoVisualScene);
    }

    Ok(model)
}

fn import_geometry(
    geo: &Geometry,
    mesh: &dae_parser::Mesh,
    all_triangles: &[&dae_parser::Triangles],
    maps: &dae_parser::LocalMaps<'_>,
    ctx: &RenderContext,
) -> Result<CpuMesh, DaeImportError> {
    let vertices = mesh.vertices.as_ref().ok_or(DaeImportError::NoTriangles)?;

    let label = Label::from(
        geo.name
            .clone()
            .or_else(|| geo.id.clone())
            .unwrap_or_default(),
    );

    let mut pos_raw = Vec::new();
    let mut normals = Vec::new();
    let mut texcoords = Vec::new();
    let mut tri_indices = Vec::<glam::UVec3>::new();
    let mut materials = SmallVec::<[mesh::Material; 1]>::new();

    for triangles in all_triangles {
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

        let vertex_offset = pos_raw.len() as u32;

        for (i, v) in dae_importer.read::<(), Vertex>(&(), prim_data).enumerate() {
            pos_raw.push(v.position);
            normals.push(v.normal);
            texcoords.push(v.texcoord);

            // Triangles are grouped in triplets
            if i % 3 == 2 {
                let base = vertex_offset + i as u32 - 2;
                tri_indices.push(glam::UVec3::new(base, base + 1, base + 2));
            }
        }

        let group_vertex_count = pos_raw.len() as u32 - vertex_offset;
        if group_vertex_count == 0 {
            continue;
        }

        let albedo_factor = triangles
            .material
            .as_ref()
            .and_then(|mat_symbol| extract_material_color(mat_symbol, maps))
            .unwrap_or(crate::Rgba::WHITE);

        materials.push(mesh::Material {
            label: label.clone(),
            index_range: vertex_offset..vertex_offset + group_vertex_count,
            albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
            albedo_factor,
        });
    }

    let num_vertices = pos_raw.len();
    let vertex_positions = bytemuck::cast_vec(pos_raw);
    let bbox = crate::util::bounding_box_from_points(vertex_positions.iter().copied());

    let cpu_mesh = mesh::CpuMesh {
        label: label.clone(),
        triangle_indices: tri_indices,
        vertex_positions,
        vertex_normals: bytemuck::cast_vec(normals),
        vertex_colors: vec![Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: bytemuck::cast_vec(texcoords),
        materials,
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

/// Compute the correction matrix that rotates from a given COLLADA `up_axis` coordinate system
/// into Rerun's default RFU (X=Right, Y=Forward, Z=Up) convention.
fn up_axis_correction(up_axis: dae_parser::UpAxis) -> glam::Affine3A {
    match up_axis {
        dae_parser::UpAxis::ZUp => glam::Affine3A::IDENTITY,
        dae_parser::UpAxis::YUp => glam::Affine3A::from_mat3(
            glam::Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2),
        ),
        // X_UP is rare and the COLLADA spec doesn't fully define the secondary axes.
        // We assume a right-handed system with X=Up, Y=Left, Z=Backward, mapping to
        // RFU as: file (x, y, z) → world (-y, -z, x).
        dae_parser::UpAxis::XUp => glam::Affine3A::from_mat3(glam::Mat3::from_cols(
            glam::Vec3::new(0.0, 0.0, 1.0),
            glam::Vec3::new(-1.0, 0.0, 0.0),
            glam::Vec3::new(0.0, -1.0, 0.0),
        )),
    }
}

/// Recursively walk the node tree, placing geometry instances with corrected world transforms.
///
/// `parent_correction` is the up-axis correction that is already baked into `parent_tf`.
/// `node_correction` is the up-axis correction that applies to *this* node's local transforms.
///
/// To convert this node's local basis into the parent's already-corrected (RFU) frame we compute
/// `pending = inverse(parent_correction) * node_correction`. For nodes that inherit the parent's
/// basis this reduces to `IDENTITY`; for nodes that override it undoes the parent's correction
/// and applies the child's.
fn gather_instances_recursive(
    model: &mut CpuModel,
    node: &DaeNode,
    parent_tf: &glam::Affine3A,
    parent_correction: &glam::Affine3A,
    node_correction: &glam::Affine3A,
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

    // Undo the parent's correction and apply this node's, so local_mat (expressed in this
    // node's basis) is correctly rebased into the parent's already-corrected RFU frame.
    let pending = parent_correction.inverse() * *node_correction;
    let world_tf = *parent_tf * pending * Affine3A::from_mat4(local_mat);

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

        if let Some(&mesh_key) = meshes.get(&id) {
            model.add_instance(mesh_key, world_tf);
        } else {
            re_log::warn_once!("<instance_geometry> references unknown geometry {id}");
        }
    }

    for child in &node.children {
        // If the child declares its own up_axis, use that; otherwise inherit this node's basis.
        let child_correction = child
            .asset
            .as_deref()
            .map(|a| up_axis_correction(a.up_axis))
            .unwrap_or(*node_correction);
        gather_instances_recursive(
            model,
            child,
            &world_tf,
            node_correction,
            &child_correction,
            meshes,
        );
    }
}

#[derive(Clone, Default)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    texcoord: [f32; 2],
}

impl<'a> VertexLoad<'a> for Vertex {
    fn position((): &(), reader: &SourceReader<'a, XYZ>, i: u32) -> Self {
        Self {
            position: reader.get(i as usize),
            normal: [0.0; 3],
            texcoord: [0.0; 2],
        }
    }

    fn add_normal(&mut self, (): &(), reader: &SourceReader<'a, XYZ>, i: u32) {
        self.normal = reader.get(i as usize);
    }

    fn add_texcoord(&mut self, (): &(), r: &SourceReader<'a, ST>, i: u32, _set: Option<u32>) {
        self.texcoord = r.get(i as usize);
    }
}
