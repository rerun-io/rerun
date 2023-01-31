//! This example demonstrates how to use the Rerun Rust SDK to log raw 3D meshes (so-called
//! "triangle soups") and their transform hierarchy.
//!

#![allow(clippy::doc_markdown)]

use std::path::PathBuf;

use anyhow::Context;
use rerun::{
    reexports::{re_log, re_memory::AccountingAllocator},
    EntityPath, LogMsg, Mesh3D, MeshId, MsgBundle, MsgId, RawMesh3D, Session, Time, TimePoint,
    TimeType, Timeline, Transform, ViewCoordinates,
};

// TODO(cmc): This example needs to support animations to showcase Rerun's time capabilities.

// --- Rerun logging ---

// Declare how to turn a glTF primitive into a Rerun component (`Mesh3D`).
impl From<GltfPrimitive> for Mesh3D {
    fn from(primitive: GltfPrimitive) -> Self {
        Mesh3D::Raw(RawMesh3D {
            mesh_id: MeshId::random(),
            indices: primitive.indices,
            positions: primitive.positions.into_iter().flatten().collect(),
            normals: primitive
                .normals
                .map(|normals| normals.into_iter().flatten().collect()),
            // TODO(cmc): We need to support vertex colors and/or texturing, otherwise it's pretty
            // hard to see anything with complex enough meshes (and hovering doesn't really help
            // when everything's white).
            // colors: primitive
            //     .colors
            //     .map(|colors| colors.into_iter().flatten().collect()),
            // texcoords: primitive
            //     .texcoords
            //     .map(|texcoords| texcoords.into_iter().flatten().collect()),
        })
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
fn log_node(session: &mut Session, node: GltfNode) {
    let ent_path = EntityPath::from(node.name.as_str());

    // What time is it?
    let timeline_keyframe = Timeline::new("keyframe", TimeType::Sequence);
    let time_point = TimePoint::from([
        // TODO(cmc): this _has_ to be inserted by the SDK
        (Timeline::log_time(), Time::now().into()),
        // TODO(cmc): animations!
        (timeline_keyframe, 0.into()),
    ]);

    // Convert glTF objects into Rerun components.
    let transform = node.transform.map(Transform::from);
    let primitives = node
        .primitives
        .into_iter()
        .map(Mesh3D::from)
        .collect::<Vec<_>>();

    // TODO(cmc): Transforms have to be logged separately because they are neither batches nor
    // splats... the user shouldn't have to know that though!
    // The SDK needs to split things up as needed when it sees a Transform component.
    //
    // We're going to have the same issue with splats: the SDK needs to automagically detect the
    // user intention to use splats and do the necessary (like the python SDK does iirc).
    if let Some(transform) = transform {
        let bundle = MsgBundle::new(
            MsgId::random(),
            ent_path.clone(),
            time_point.clone(),
            // TODO(cmc): need to reproduce the viewer crash I had earlier and fix/log-an-issue for
            // it.
            vec![vec![transform].try_into().unwrap()],
        );
        // TODO(cmc): These last conversion details need to be hidden in the SDK.
        let msg = bundle.try_into().unwrap();
        session.send(LogMsg::ArrowMsg(msg));
    }

    // TODO(cmc): Working at the `ComponentBundle`/`TryIntoArrow` layer feels too low-level,
    // something like a MsgBuilder kinda thing would probably be quite nice.
    let bundle = MsgBundle::new(
        MsgId::random(),
        ent_path,
        time_point,
        vec![primitives.try_into().unwrap()],
    );

    // Create and send one message to the sdk
    // TODO(cmc): These last conversion details need to be hidden in the SDK.
    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));

    // Recurse through all of the node's children!
    for mut child in node.children {
        child.name = [node.name.as_str(), child.name.as_str()].join("/");
        log_node(session, child);
    }
}

// TODO(cmc): The SDK should make this call so trivial that it doesn't require this helper at all.
fn log_axis(session: &mut Session, ent_path: &EntityPath) {
    // From the glTF spec:
    // > glTF uses a right-handed coordinate system. glTF defines +Y as up, +Z as forward, and
    // > -X as right; the front of a glTF asset faces +Z.
    let view_coords: ViewCoordinates = "RUB".parse().unwrap();

    let bundle = MsgBundle::new(
        MsgId::random(),
        ent_path.clone(),
        [].into(), // TODO(cmc): doing timeless stuff shouldn't be so weird
        vec![vec![view_coords].try_into().unwrap()],
    );

    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));
}

// --- Init ---

// TODO(cmc): This should probably just be a compile flag of the SDK; if it's enabled, then the
// global allocator is properly set by the SDK directly.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    // TODO(cmc): Here we shall pass argv to the SDK which will strip it out of all SDK flags, and
    // give us back our actual CLI flags.
    // The name of the gltf sample to load should then come from there.

    // Load glTF asset
    let bytes = download_gltf_sample("Buggy").unwrap();

    // Parse glTF asset
    let (doc, buffers, _) = gltf::import_slice(bytes).unwrap();
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    let mut session = Session::new();
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        log_axis(&mut session, &root.name.as_str().into());
        log_node(&mut session, root);
    }

    // TODO(cmc): provide high-level tools to pick and handle the different modes.
    // TODO(cmc): connect, spawn_and_connect; show() probably doesn't make sense with pure rust
    let log_messages = session.drain_log_messages_buffer();
    rerun::viewer::show(log_messages).context("failed to start viewer")
}

// --- glTF parsing ---

struct GltfNode {
    name: String,
    transform: Option<GltfTransform>,
    primitives: Vec<GltfPrimitive>,
    children: Vec<GltfNode>,
}

struct GltfPrimitive {
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
        // TODO(cmc): now that index-paths and instance-keys are decorrelated, maybe we can use
        // those here.
        .map_or_else(|| format!("node_#{}", node.index()), ToOwned::to_owned)
}

fn node_primitives<'data>(
    buffers: &'data [gltf::buffer::Data],
    node: &'data gltf::Node<'_>,
) -> impl Iterator<Item = GltfPrimitive> + 'data {
    node.mesh().into_iter().flat_map(|mesh| {
        mesh.primitives().map(|primitive| {
            assert!(primitive.mode() == gltf::mesh::Mode::Triangles);

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
                positions,
                indices,
                normals,
                colors,
                texcoords,
            }
        })
    })
}

// TODO(cmc): This is unfortunately _not_ the right way to load a glTF scene.
// We need to load the the default skinning transforms if they exist, and traverse the scene
// differently in that case.
// It's apparently very common for glTF meshes in the wild to define default skinning transforms
// for their initial orientation.
fn load_gltf<'data>(
    doc: &'data gltf::Document,
    buffers: &'data [gltf::buffer::Data],
) -> impl Iterator<Item = GltfNode> + 'data {
    doc.scenes().map(move |scene| {
        let name = scene
            .name()
            // TODO(cmc): now that index-paths and instance-keys are decorrelated, maybe we can use
            // those here.
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

// --- Assets ---

// TODO(cmc): This needs to be implemented and exposed by the SDK (probably behind a feature flag),
// and can probably be re-used by the Python examples too.
fn download_gltf_sample(name: impl AsRef<str>) -> anyhow::Result<bytes::Bytes> {
    const GLTF_SAMPLE_URL: &str = "https://github.com/KhronosGroup/glTF-Sample-Models/blob/db9ff67c1116cfe28eb36320916bccd8c4127cc1/2.0/_NAME_/glTF-Binary/_NAME_.glb?raw=true";

    let url = GLTF_SAMPLE_URL.replace("_NAME_", name.as_ref());

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dataset/samples");
    let path = dir.join(format!("{}.glb", name.as_ref().to_lowercase()));

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create asset directory: {dir:?}"))?;

    if let Ok(bytes) = std::fs::read(&path) {
        re_log::info!(asset = ?path, "loading asset from disk cache...");
        Ok(bytes::Bytes::from(bytes))
    } else {
        re_log::info!(asset = ?path, "loading asset from network...");
        let res = reqwest::blocking::get(&url)
            .with_context(|| format!("failed to fetch asset: {url}"))?;
        let bytes = res
            .bytes()
            .with_context(|| format!("failed to fetch asset: {url}"))?;
        std::fs::write(&path, &bytes)
            .with_context(|| format!("failed to write asset to disk: {path:?}"))?;
        Ok(bytes)
    }
}
