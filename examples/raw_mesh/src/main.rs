//! This example demonstrates how to use the Rerun Rust SDK to log raw 3D meshes (so-called
//! "triangle soups") and their transform hierarchy.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh <path_to_gltf_scene>
//! ```

#![allow(clippy::doc_markdown)]

use anyhow::{bail, Context};
use bytes::Bytes;
use rerun::{
    reexports::{re_log, re_memory::AccountingAllocator},
    EntityPath, LogMsg, Mesh3D, MeshId, MsgBundle, MsgId, RawMesh3D, Session, Time, TimePoint,
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
    // glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
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

// Use MiMalloc as global allocator (because it is fast), wrapped in Rerun's allocation tracker
// so that the rerun viewer can show how much memory it is using when calling `show`.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    // TODO(cmc): Here we shall pass argv to the SDK which will strip it out of all SDK flags, and
    // give us back our actual CLI flags.
    // The name of the gltf sample to load should then come from there.

    // Read glTF asset
    let args = std::env::args().collect::<Vec<_>>();
    let bytes = if let Some(path) = args.get(1) {
        Bytes::from(std::fs::read(path)?)
    } else {
        bail!("Usage: {} <path_to_gltf_scene>", args[0]);
    };

    // Parse glTF asset
    let (doc, buffers, _) = gltf::import_slice(bytes).unwrap();
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    let mut session = Session::new();
    session.connect("127.0.0.1:9876".parse().unwrap());
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        log_axis(&mut session, &root.name.as_str().into());
        log_node(&mut session, root);
    }

    session.flush();

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
