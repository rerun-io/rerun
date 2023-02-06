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
use clap::Parser;
use rerun::{
    external::{re_log, re_memory::AccountingAllocator},
    ApplicationId, EntityPath, Mesh3D, MeshId, MsgSender, RawMesh3D, RecordingId, Session,
    TimeType, Timeline, Transform, Vec4D, ViewCoordinates,
};

// TODO(cmc): This example needs to support animations to showcase Rerun's time capabilities.

// --- Rerun logging ---

// Declare how to turn a glTF primitive into a Rerun component (`Mesh3D`).
#[allow(clippy::fallible_impl_from)]
impl From<GltfPrimitive> for Mesh3D {
    fn from(primitive: GltfPrimitive) -> Self {
        let raw = RawMesh3D {
            mesh_id: MeshId::random(),
            albedo_factor: primitive.albedo_factor.map(Vec4D),
            indices: primitive.indices,
            positions: primitive.positions.into_iter().flatten().collect(),
            normals: primitive
                .normals
                .map(|normals| normals.into_iter().flatten().collect()),
            //
            // TODO(cmc): We need to support vertex colors and/or texturing, otherwise it's pretty
            // hard to see anything with complex enough meshes (and hovering doesn't really help
            // when everything's white).
            // colors: primitive
            //     .colors
            //     .map(|colors| colors.into_iter().flatten().collect()),
            // texcoords: primitive
            //     .texcoords
            //     .map(|texcoords| texcoords.into_iter().flatten().collect()),
        };

        raw.sanity_check().unwrap();

        Mesh3D::Raw(raw)
    }
}

// Declare how to turn a glTF transform into a Rerun component (`Transform`).
impl From<GltfTransform> for Transform {
    fn from(transform: GltfTransform) -> Self {
        Transform::Rigid3(rerun::Rigid3 {
            rotation: rerun::Quaternion {
                x: transform.r[0],
                y: transform.r[1],
                z: transform.r[2],
                w: transform.r[3],
            },
            translation: rerun::Vec3D(transform.t),
        })
    }
}

/// Log a glTF node with Rerun.
fn log_node(session: &mut Session, node: GltfNode) -> anyhow::Result<()> {
    let ent_path = EntityPath::from(node.name.as_str());

    // Convert glTF objects into Rerun components.
    let transform = node.transform.map(Transform::from);
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
        .send(session)?;

    // Recurse through all of the node's children!
    for mut child in node.children {
        child.name = [node.name.as_str(), child.name.as_str()].join("/");
        log_node(session, child)?;
    }

    Ok(())
}

fn log_coordinate_space(
    session: &mut Session,
    ent_path: impl Into<EntityPath>,
    axes: &str,
) -> anyhow::Result<()> {
    let view_coords: ViewCoordinates = axes
        .parse()
        .map_err(|err| anyhow!("couldn't parse {axes:?} as ViewCoordinates: {err}"))?;

    MsgSender::new(ent_path)
        .with_timeless(true)
        .with_component(&[view_coords])?
        .send(session)
        .map_err(Into::into)
}

// --- Init ---

// Use MiMalloc as global allocator (because it is fast), wrapped in Rerun's allocation tracker
// so that the rerun viewer can show how much memory it is using when calling `show`.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// If specified, connects and sends the logged data to a remote Rerun viewer.
    ///
    /// Optionally takes an ip:port, otherwise uses Rerun's defaults.
    #[clap(long)]
    #[allow(clippy::option_option)]
    connect: Option<Option<String>>,

    /// Specifies the path of the glTF scene to load.
    #[clap(long)]
    scene_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    let args = Args::parse();
    let addr = match args.connect.as_ref() {
        Some(Some(addr)) => Some(addr.parse()?),
        Some(None) => Some(rerun::default_server_addr()),
        None => None,
    };

    let mut session = Session::new();
    // TODO(cmc): The Rust SDK needs a higher-level `init()` method, akin to what the python SDK
    // does... which they can probably share.
    // This needs to take care of the whole `official_example` thing, and also keeps track of
    // whether we're using the rust or python sdk.
    session.set_application_id(ApplicationId("objectron-rs".into()), true);
    session.set_recording_id(RecordingId::random());
    if let Some(addr) = addr {
        session.connect(addr);
    }

    // Read glTF scene
    let (doc, buffers, _) = gltf::import_slice(Bytes::from(std::fs::read(args.scene_path)?))?;
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        log_coordinate_space(&mut session, root.name.as_str(), "RUB")?;
        log_node(&mut session, root)?;
    }

    // TODO(cmc): arg parsing and arg interpretation helpers
    // TODO(cmc): missing flags: save, serve
    // TODO(cmc): expose an easy to use async local mode.
    if args.connect.is_none() {
        let log_messages = session.drain_log_messages_buffer();
        rerun::viewer::show(log_messages)?;
    }

    Ok(())
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
    positions: Vec<[f32; 3]>,
    indices: Option<Vec<u32>>,
    normals: Option<Vec<[f32; 3]>>,
    #[allow(dead_code)]
    colors: Option<Vec<[u8; 4]>>,
    #[allow(dead_code)]
    texcoords: Option<Vec<[f32; 2]>>,
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

            let positions = reader.read_positions().unwrap();
            let positions = positions.collect();

            let indices = reader.read_indices();
            let indices = indices.map(|indices| indices.into_u32().into_iter().collect());

            let normals = reader.read_normals();
            let normals = normals.map(|normals| normals.collect());

            let colors = reader.read_colors(0); // TODO(cmc): pick correct set
            let colors = colors.map(|colors| colors.into_rgba_u8().collect());

            let texcoords = reader.read_tex_coords(0); // TODO(cmc): pick correct set
            let texcoords = texcoords.map(|texcoords| texcoords.into_f32().collect());

            GltfPrimitive {
                albedo_factor,
                positions,
                indices,
                normals,
                colors,
                texcoords,
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
