//! This example demonstrates how to use the Rerun Rust SDK to log raw 3D meshes (so-called
//! "triangle soups") and their transform hierarchy.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh <path_to_gltf_scene>
//! ```

#![allow(clippy::doc_markdown)]

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::anyhow;
use bytes::Bytes;
use rerun::components::{Mesh3D, MeshId, RawMesh3D, Transform, Vec4D, ViewCoordinates};
use rerun::time::{TimeType, Timeline};
use rerun::{
    external::{re_log, re_memory::AccountingAllocator},
    ApplicationId, EntityPath, MsgSender, RecordingId, Session,
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
        Transform::Rigid3(rerun::components::Rigid3 {
            rotation: rerun::components::Quaternion {
                x: transform.r[0],
                y: transform.r[1],
                z: transform.r[2],
                w: transform.r[3],
            },
            translation: rerun::components::Vec3D(transform.t),
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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Scene {
    Buggy,
    #[value(name("brain_stem"))]
    BrainStem,
    Lantern,
    Avocado,
}

#[derive(Debug, Clone)]
enum Behavior {
    Save(PathBuf),
    Serve,
    Connect(SocketAddr),
    Spawn,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Start a viewer and feed it data in real-time.
    #[clap(long, default_value = "true")]
    spawn: bool,

    /// Saves the data to an rrd file rather than visualizing it immediately.
    #[clap(long)]
    save: Option<PathBuf>,

    /// If specified, connects and sends the logged data to a remote Rerun viewer.
    ///
    /// Optionally takes an ip:port, otherwise uses Rerun's defaults.
    #[clap(long)]
    #[allow(clippy::option_option)]
    connect: Option<Option<SocketAddr>>,

    /// Connects and sends the logged data to a web-based Rerun viewer.
    #[clap(long)]
    serve: bool,

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

    // TODO(cmc): move all rerun args handling to helpers
    pub fn to_behavior(&self) -> Behavior {
        if let Some(path) = self.save.as_ref() {
            return Behavior::Save(path.clone());
        }

        if self.serve {
            return Behavior::Serve;
        }

        match self.connect {
            Some(Some(addr)) => return Behavior::Connect(addr),
            Some(None) => return Behavior::Connect(rerun::log::default_server_addr()),
            None => {}
        }

        Behavior::Spawn
    }
}

fn run(session: &mut Session, args: &Args) -> anyhow::Result<()> {
    // Read glTF scene
    let (doc, buffers, _) = gltf::import_slice(Bytes::from(std::fs::read(args.scene_path()?)?))?;
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        log_coordinate_space(session, root.name.as_str(), "RUB")?;
        log_node(session, root)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let mut session = Session::new();
    // TODO(cmc): The Rust SDK needs a higher-level `init()` method, akin to what the python SDK
    // does... which they can probably share.
    // This needs to take care of the whole `official_example` thing, and also keeps track of
    // whether we're using the rust or python sdk.
    session.set_application_id(ApplicationId("raw_mesh_rs".into()), true);
    session.set_recording_id(RecordingId::random());

    let behavior = args.to_behavior();
    match behavior {
        Behavior::Connect(addr) => session.connect(addr),
        Behavior::Spawn => {
            return session
                .spawn(move |mut session| run(&mut session, &args))
                .map_err(Into::into)
        }
        Behavior::Serve => session.serve(true),
        Behavior::Save(_) => {}
    }

    run(&mut session, &args)?;

    if matches!(behavior, Behavior::Serve) {
        eprintln!("Sleeping while serving the web viewer. Abort with Ctrl-C");
        std::thread::sleep(std::time::Duration::from_secs(1_000_000));
    } else if let Behavior::Save(path) = behavior {
        session.save(path)?;
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
