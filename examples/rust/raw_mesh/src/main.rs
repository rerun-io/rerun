//! This example demonstrates how to use the Rerun Rust SDK to log raw 3D meshes (so-called
//! "triangle soups") and their transform hierarchy.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh <path_to_gltf_scene>
//! ```

#![allow(clippy::doc_markdown)]

use std::path::PathBuf;

use anyhow::anyhow;
use bytes::Bytes;
use rerun::components::{
    ColorRGBA, Mesh3D, MeshId, RawMesh3D, Transform3D, Vec4D, ViewCoordinates,
};
use rerun::time::{TimeType, Timeline};
use rerun::transform::TranslationRotationScale3D;
use rerun::{
    external::{re_log, re_memory::AccountingAllocator},
    EntityPath, MsgSender, RecordingStream,
};

// TODO(cmc): This example needs to support animations to showcase Rerun's time capabilities.

// --- Rerun logging ---

// Declare how to turn a glTF primitive into a Rerun component (`Mesh3D`).
#[allow(clippy::fallible_impl_from)]
impl From<GltfPrimitive> for Mesh3D {
    fn from(primitive: GltfPrimitive) -> Self {
        let GltfPrimitive {
            albedo_factor,
            indices,
            vertex_positions,
            vertex_colors,
            vertex_normals,
            vertex_texcoords: _, // TODO(cmc) support mesh texturing
        } = primitive;

        let raw = RawMesh3D {
            mesh_id: MeshId::random(),
            albedo_factor: albedo_factor.map(Vec4D),
            indices: indices.map(|i| i.into()),
            vertex_positions: vertex_positions.into_iter().flatten().collect(),
            vertex_normals: vertex_normals.map(|normals| normals.into_iter().flatten().collect()),
            vertex_colors: vertex_colors.map(|colors| colors.into_iter().map(|c| c.0).collect()),
        };

        raw.sanity_check().unwrap();

        Mesh3D::Raw(raw)
    }
}

// Declare how to turn a glTF transform into a Rerun component (`Transform`).
impl From<GltfTransform> for Transform3D {
    fn from(transform: GltfTransform) -> Self {
        Transform3D::new(TranslationRotationScale3D::affine(
            transform.t,
            rerun::components::Quaternion::from_xyzw(transform.r),
            transform.s,
        ))
    }
}

/// Log a glTF node with Rerun.
fn log_node(rec_stream: &RecordingStream, node: GltfNode) -> anyhow::Result<()> {
    let ent_path = EntityPath::from(node.name.as_str());

    // Convert glTF objects into Rerun components.
    let transform = node.transform.map(Transform3D::from);
    let primitives = node
        .primitives
        .into_iter()
        .map(Mesh3D::from)
        .collect::<Vec<_>>();

    let timeline_keyframe = Timeline::new("keyframe", TimeType::Sequence);
    MsgSender::new(ent_path)
        .with_time(timeline_keyframe, 0)
        .with_component(&primitives)?
        .with_component(transform.as_ref())?
        .send(rec_stream)?;

    // Recurse through all of the node's children!
    for mut child in node.children {
        child.name = [node.name.as_str(), child.name.as_str()].join("/");
        log_node(rec_stream, child)?;
    }

    Ok(())
}

fn log_coordinate_space(
    rec_stream: &RecordingStream,
    ent_path: impl Into<EntityPath>,
    axes: &str,
) -> anyhow::Result<()> {
    let view_coords: ViewCoordinates = axes
        .parse()
        .map_err(|err| anyhow!("couldn't parse {axes:?} as ViewCoordinates: {err}"))?;

    MsgSender::new(ent_path)
        .with_timeless(true)
        .with_component(&[view_coords])?
        .send(rec_stream)
        .map_err(Into::into)
}

// --- Init ---

// Use MiMalloc as global allocator (because it is fast), wrapped in Rerun's allocation tracker
// so that the rerun viewer can show how much memory it is using when calling `show`.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Scene {
    Buggy,
    #[value(name("brain_stem"))]
    BrainStem,
    Lantern,
    Avocado,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Specifies the glTF scene to load.
    #[clap(long, value_enum, default_value = "buggy")]
    scene: Scene,

    /// Specifies the path of an arbitrary glTF scene to load.
    #[clap(long)]
    scene_path: Option<PathBuf>,
}

// TODO(cmc): move all rerun args handling to helpers
impl Args {
    fn scene_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(scene_path) = self.scene_path.clone() {
            return Ok(scene_path);
        }

        const DATASET_DIR: &str =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../python/raw_mesh/dataset");

        use clap::ValueEnum as _;
        let scene = self.scene.to_possible_value().unwrap();
        let scene_name = scene.get_name();

        let scene_path = PathBuf::from(DATASET_DIR)
            .join(scene_name)
            .join(format!("{scene_name}.glb"));
        if !scene_path.exists() {
            anyhow::bail!(
                "Could not load the scene, have you downloaded the dataset? \
                Try running the python version first to download it automatically \
                (`examples/python/raw_mesh/main.py --scene {scene_name}`).",
            )
        }

        Ok(scene_path)
    }
}

fn run(rec_stream: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    // Read glTF scene
    let (doc, buffers, _) = gltf::import_slice(Bytes::from(std::fs::read(args.scene_path()?)?))?;
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        log_coordinate_space(rec_stream, root.name.as_str(), "RUB")?;
        log_node(rec_stream, root)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun
        .clone()
        .run("raw_mesh_rs", default_enabled, move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        })
}

// --- glTF parsing ---

struct GltfNode {
    name: String,
    transform: Option<GltfTransform>,
    primitives: Vec<GltfPrimitive>,
    children: Vec<GltfNode>,
}

struct GltfPrimitive {
    albedo_factor: Option<[f32; 4]>,
    indices: Option<Vec<u32>>,
    vertex_positions: Vec<[f32; 3]>,
    vertex_colors: Option<Vec<ColorRGBA>>,
    vertex_normals: Option<Vec<[f32; 3]>>,
    #[allow(dead_code)]
    vertex_texcoords: Option<Vec<[f32; 2]>>,
}

struct GltfTransform {
    t: [f32; 3],
    r: [f32; 4],
    #[allow(dead_code)]
    s: [f32; 3],
}

impl GltfNode {
    fn from_gltf(buffers: &[gltf::buffer::Data], node: &gltf::Node<'_>) -> Self {
        let name = node_name(node);

        let transform = {
            let (t, r, s) = node.transform().decomposed();
            GltfTransform { t, r, s }
        };
        let primitives = node_primitives(buffers, node).collect();

        let children = node
            .children()
            .map(|child| GltfNode::from_gltf(buffers, &child))
            .collect();

        Self {
            name,
            transform: Some(transform),
            primitives,
            children,
        }
    }
}

fn node_name(node: &gltf::Node<'_>) -> String {
    node.name()
        .map_or_else(|| format!("node_#{}", node.index()), ToOwned::to_owned)
}

fn node_primitives<'data>(
    buffers: &'data [gltf::buffer::Data],
    node: &'data gltf::Node<'_>,
) -> impl Iterator<Item = GltfPrimitive> + 'data {
    node.mesh().into_iter().flat_map(|mesh| {
        mesh.primitives().map(|primitive| {
            assert!(primitive.mode() == gltf::mesh::Mode::Triangles);

            let albedo_factor = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor()
                .into();

            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let indices = reader.read_indices();
            let indices = indices.map(|indices| indices.into_u32().collect());

            let vertex_positions = reader.read_positions().unwrap();
            let vertex_positions = vertex_positions.collect();

            let vertex_normals = reader.read_normals();
            let vertex_normals = vertex_normals.map(|normals| normals.collect());

            let vertex_colors = reader.read_colors(0); // TODO(cmc): pick correct set
            let vertex_colors = vertex_colors.map(|colors| {
                colors
                    .into_rgba_u8()
                    .map(|[r, g, b, a]| ColorRGBA::from_unmultiplied_rgba(r, g, b, a))
                    .collect()
            });

            let vertex_texcoords = reader.read_tex_coords(0); // TODO(cmc): pick correct set
            let vertex_texcoords = vertex_texcoords.map(|texcoords| texcoords.into_f32().collect());

            GltfPrimitive {
                albedo_factor,
                vertex_positions,
                indices,
                vertex_normals,
                vertex_colors,
                vertex_texcoords,
            }
        })
    })
}

fn load_gltf<'data>(
    doc: &'data gltf::Document,
    buffers: &'data [gltf::buffer::Data],
) -> impl Iterator<Item = GltfNode> + 'data {
    doc.scenes().map(move |scene| {
        let name = scene
            .name()
            .map_or_else(|| format!("scene_#{}", scene.index()), ToOwned::to_owned);

        re_log::info!(scene = name, "parsing glTF scene");

        GltfNode {
            name,
            transform: None,
            primitives: Default::default(),
            children: scene
                .nodes()
                .map(|node| GltfNode::from_gltf(buffers, &node))
                .collect(),
        }
    })
}
