//! This example demonstrates how to use the Rerun Rust SDK to log raw 3D meshes (so-called
//! "triangle soups") and their transform hierarchy.
//!
//! Note that while this example loads GLTF meshes to illustrate
//! [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d)'s abilitites,
//! you can also send various kinds of mesh assets
//! directly via [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh <path_to_gltf_scene>
//! ```

use std::path::PathBuf;

use bytes::Bytes;
use rerun::external::re_log;
use rerun::{Color, Mesh3D, RecordingStream, Rgba32};

// TODO(cmc): This example needs to support animations to showcase Rerun's time capabilities.

// --- Rerun logging ---

// Declare how to turn a glTF primitive into a Rerun component (`Mesh3D`).
#[expect(clippy::fallible_impl_from)]
impl From<GltfPrimitive> for Mesh3D {
    fn from(primitive: GltfPrimitive) -> Self {
        let GltfPrimitive {
            albedo_factor,
            indices,
            vertex_positions,
            vertex_colors,
            vertex_normals,
            vertex_texcoords,
        } = primitive;

        let mut mesh = Mesh3D::new(vertex_positions);

        if let Some(indices) = indices {
            assert!(indices.len() % 3 == 0);
            let triangle_indices = indices.chunks_exact(3).map(|tri| (tri[0], tri[1], tri[2]));
            mesh = mesh.with_triangle_indices(triangle_indices);
        }
        if let Some(vertex_normals) = vertex_normals {
            mesh = mesh.with_vertex_normals(vertex_normals);
        }
        if let Some(vertex_colors) = vertex_colors {
            mesh = mesh.with_vertex_colors(vertex_colors);
        }
        if let Some(vertex_texcoords) = vertex_texcoords {
            mesh = mesh.with_vertex_texcoords(vertex_texcoords);
        }
        if let Some([r, g, b, a]) = albedo_factor {
            mesh = mesh.with_albedo_factor(Rgba32::from_linear_unmultiplied_rgba_f32(r, g, b, a));
        }

        mesh.sanity_check().unwrap();

        mesh
    }
}

// Declare how to turn a glTF transform into a Rerun component (`Transform`).
impl From<GltfTransform> for rerun::Transform3D {
    fn from(transform: GltfTransform) -> Self {
        rerun::Transform3D::from_translation_rotation_scale(
            transform.t,
            rerun::datatypes::Quaternion::from_xyzw(transform.r),
            transform.s,
        )
    }
}

/// Log a glTF node with Rerun.
fn log_node(rec: &RecordingStream, node: GltfNode) -> anyhow::Result<()> {
    rec.set_time_sequence("keyframe", 0);

    if let Some(transform) = node.transform.map(rerun::Transform3D::from) {
        rec.log(node.name.as_str(), &transform)?;
    }

    // Convert glTF objects into Rerun components.
    for (i, primitive) in node.primitives.into_iter().enumerate() {
        let mesh: Mesh3D = primitive.into();
        rec.log(format!("{}/{}", node.name, i), &mesh)?;
    }

    // Recurse through all of the node's children!
    for mut child in node.children {
        child.name = [node.name.as_str(), child.name.as_str()].join("/");
        log_node(rec, child)?;
    }

    Ok(())
}

// --- Init ---

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
                (`python -m raw_mesh --scene {scene_name}`).",
            )
        }

        Ok(scene_path)
    }
}

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    // Read glTF scene
    let (doc, buffers, _) = gltf::import_slice(Bytes::from(std::fs::read(args.scene_path()?)?))?;
    let nodes = load_gltf(&doc, &buffers);

    // Log raw glTF nodes and their transforms with Rerun
    for root in nodes {
        re_log::info!(scene = root.name, "logging glTF scene");
        rec.log_static(
            root.name.as_str(),
            &rerun::ViewCoordinates::RIGHT_HAND_Y_UP(),
        )?;
        log_node(rec, root)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_raw_mesh")?;
    run(&rec, &args)
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
    vertex_colors: Option<Vec<Color>>,
    vertex_normals: Option<Vec<[f32; 3]>>,
    vertex_texcoords: Option<Vec<[f32; 2]>>,
}

struct GltfTransform {
    t: [f32; 3],
    r: [f32; 4],
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
        .map_or_else(|| format!("node_{}", node.index()), ToOwned::to_owned)
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
                    .map(|[r, g, b, a]| Color::from_unmultiplied_rgba(r, g, b, a))
                    .collect()
            });

            let vertex_texcoords = reader.read_tex_coords(0); // TODO(cmc): pick correct set
            let vertex_texcoords = vertex_texcoords.map(|texcoords| texcoords.into_f32().collect());

            // TODO(cmc): support for albedo textures

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
            .map_or_else(|| format!("scene_{}", scene.index()), ToOwned::to_owned);

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
