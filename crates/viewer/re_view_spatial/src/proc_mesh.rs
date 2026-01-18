//! Procedurally-generated meshes for rendering objects that are
//! specified geometrically, and have nontrivial numbers of vertices each,
//! such as a sphere or cylinder.

use std::sync::Arc;

use glam::{Vec3, Vec3A, uvec3, vec3};
use hexasphere::{BaseShape, Subdivided};
use itertools::Itertools as _;
use macaw::MeshGen;
use ordered_float::NotNan;
use re_byte_size::SizeBytes as _;
use re_chunk_store::external::re_chunk::external::re_byte_size;
use re_renderer::RenderContext;
use re_renderer::mesh::{self, GpuMesh, MeshError};
use re_viewer_context::Cache;
use smallvec::smallvec;

// ----------------------------------------------------------------------------

/// Description of a mesh that can be procedurally generated.
///
/// Obtain the actual mesh by passing this to [`WireframeCache`] or [`SolidCache`].
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ProcMeshKey {
    /// A unit cube, centered; its bounds are ±0.5.
    Cube,

    /// A sphere of unit radius.
    ///
    /// The resulting mesh may be scaled to represent spheres and ellipsoids
    /// of other sizes.
    Sphere {
        /// Number of triangle subdivisions to perform to create a finer, rounder mesh.
        ///
        /// If this number is zero, then the “sphere” is an octahedron. Increasing it to N
        /// breaks the edges of that octahedron into N segments. Around a great circle of the
        /// sphere, there are (N + 1) × 4 segments.
        subdivisions: usize,

        /// If true, then when a wireframe mesh is generated, it includes only
        /// the 3 axis-aligned “equatorial” circles, and not the full triangle mesh.
        axes_only: bool,
    },

    /// A capsule; a cylinder with hemispherical end caps.
    ///
    /// The capsule always has radius 1. It should be scaled to obtain the desired radius.
    /// It always extends along the positive direction of the Z axis.
    Capsule {
        /// The length of the capsule; the distance between the centers of its endpoints.
        /// This length must be non-negative.
        //
        // TODO(#1361): This is a bad approach to rendering capsules of arbitrary
        // length, because it fills the cache with many distinct meshes.
        // Instead, the renderers should be extended to support “bones” such that a mesh
        // can have parts which are independently offset, thus allowing us to stretch a
        // single sphere/capsule mesh into an arbitrary length and radius capsule.
        // (Tapered capsules will still need distinct meshes.)
        length: NotNan<f32>,

        /// Number of triangle subdivisions to use to create a finer, rounder mesh.
        ///
        /// The cylinder part of the capsule is approximated as a mesh with (N + 1) × 4
        /// flat faces.
        subdivisions: usize,

        /// If true, wireframe meshes are generated with reduced complexity,
        /// showing a minimal set of lines that outline the shape.
        axes_only: bool,
    },

    /// The cylinder always has radius 1. It should be scaled to obtain the desired radius.
    /// It always extends along the positive direction of the Z axis.
    Cylinder {
        /// Number of triangle subdivisions to use to create a finer, rounder mesh.
        ///
        /// The cylinder is approximated as a mesh with (N + 1) × 4 flat faces.
        subdivisions: usize,

        /// If true, wireframe meshes are generated with reduced complexity,
        /// showing a minimal set of lines that outline the shape.
        axes_only: bool,
    },
}

impl re_byte_size::SizeBytes for ProcMeshKey {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl ProcMeshKey {
    /// Returns the bounding box which can be computed from the mathematical shape,
    /// without regard for its exact approximation as a mesh.
    pub fn simple_bounding_box(&self) -> macaw::BoundingBox {
        match self {
            Self::Sphere {
                subdivisions: _,
                axes_only: _,
            } => {
                // sphere’s radius is 1, so its size is 2
                macaw::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(2.0))
            }
            Self::Cube => macaw::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(1.0)),
            Self::Capsule {
                subdivisions: _,
                axes_only: _,
                length,
            } => macaw::BoundingBox::from_min_max(
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0 + length.into_inner()),
            ),
            Self::Cylinder {
                subdivisions: _,
                axes_only: _,
            } => {
                // cylinder's radius is 1, so its size is 2
                macaw::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(2.0))
            }
        }
    }
}

/// A renderable mesh generated from a [`ProcMeshKey`] by the [`WireframeCache`],
/// which is to be drawn as lines rather than triangles.
#[derive(Debug)]
pub struct WireframeMesh {
    #[expect(unused)]
    pub bbox: macaw::BoundingBox,

    #[expect(unused)]
    pub vertex_count: usize,

    /// Collection of line strips making up the wireframe.
    ///
    /// TODO(kpreid): This should instead be a GPU buffer, but we don’t yet have a
    /// `re_renderer::Renderer` implementation that takes instanced meshes and applies
    /// the line shader to them, instead of doing immediate-mode accumulation of line strips.
    pub line_strips: Vec<Vec<Vec3>>,
}

impl re_byte_size::SizeBytes for WireframeMesh {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            bbox: _,
            vertex_count: _,
            line_strips,
        } = self;
        line_strips
            .iter()
            .map(|strip| strip.len() * std::mem::size_of::<Vec3>())
            .sum::<usize>() as _
    }
}

/// A renderable mesh generated from a [`ProcMeshKey`] by the [`SolidCache`],
/// which is to be drawn as triangles rather than lines.
///
/// This type is cheap to clone.
#[derive(Clone)]
pub struct SolidMesh {
    #[expect(unused)]
    pub bbox: macaw::BoundingBox,

    /// Mesh to render. Note that its colors are set to black, so that the
    /// `MeshInstance::additive_tint` can be used to set the color per instance.
    pub gpu_mesh: Arc<GpuMesh>,
}

impl re_byte_size::SizeBytes for SolidMesh {
    fn heap_size_bytes(&self) -> u64 {
        0 // Mostly VRAM
    }
}

/// Errors that may arise from attempting to generate a mesh from a [`ProcMeshKey`].
///
/// Currently, this type is private because errors are only logged.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
enum GenError {
    /// Either the GPU mesh could not be allocated,
    /// or the generated mesh was not well-formed.
    #[error(transparent)]
    MeshProcessing(#[from] MeshError),
}

// ----------------------------------------------------------------------------

/// Cache for the computation of wireframe meshes from [`ProcMeshKey`]s.
/// These meshes may then be rendered as instances of the cached
/// mesh.
#[derive(Default)]
pub struct WireframeCache(ahash::HashMap<ProcMeshKey, Option<Arc<WireframeMesh>>>);

impl WireframeCache {
    pub fn entry(
        &mut self,
        key: ProcMeshKey,
        render_ctx: &RenderContext,
    ) -> Option<Arc<WireframeMesh>> {
        self.0
            .entry(key)
            .or_insert_with(|| {
                re_tracing::profile_scope!("proc_mesh::WireframeCache(miss)", format!("{key:?}"));

                re_log::trace!("Generating wireframe mesh {key:?}…");

                Some(Arc::new(generate_wireframe(&key, render_ctx)))
            })
            .clone()
    }
}

impl Cache for WireframeCache {
    fn name(&self) -> &'static str {
        "Proc Mesh Wireframes"
    }

    fn purge_memory(&mut self) {
        self.0.clear();
    }
}

impl re_byte_size::MemUsageTreeCapture for WireframeCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}

/// Generate a wireframe mesh without caching.
///
/// Note: The unstructured error type here is used only for logging.
fn generate_wireframe(key: &ProcMeshKey, render_ctx: &RenderContext) -> WireframeMesh {
    re_tracing::profile_function!();

    // In the future, render_ctx will be used to allocate GPU memory for the mesh.
    _ = render_ctx;

    match *key {
        ProcMeshKey::Cube => {
            let corners = [
                vec3(-0.5, -0.5, -0.5),
                vec3(-0.5, -0.5, 0.5),
                vec3(-0.5, 0.5, -0.5),
                vec3(-0.5, 0.5, 0.5),
                vec3(0.5, -0.5, -0.5),
                vec3(0.5, -0.5, 0.5),
                vec3(0.5, 0.5, -0.5),
                vec3(0.5, 0.5, 0.5),
            ];
            let line_strips: Vec<Vec<Vec3>> = vec![
                // bottom:
                vec![
                    // bottom loop
                    corners[0b000],
                    corners[0b001],
                    corners[0b011],
                    corners[0b010],
                    corners[0b000],
                    // joined to top loop
                    corners[0b100],
                    corners[0b101],
                    corners[0b111],
                    corners[0b110],
                    corners[0b100],
                ],
                // remaining side edges
                vec![corners[0b001], corners[0b101]],
                vec![corners[0b010], corners[0b110]],
                vec![corners[0b011], corners[0b111]],
            ];
            WireframeMesh {
                bbox: key.simple_bounding_box(),
                vertex_count: line_strips.iter().map(|v| v.len()).sum(),
                line_strips,
            }
        }
        ProcMeshKey::Sphere {
            subdivisions,
            axes_only,
        } => {
            let subdiv: hexasphere::Subdivided<(), OctahedronBase> =
                hexasphere::Subdivided::new(subdivisions, |_| ());

            let sphere_points = subdiv.raw_points();

            let line_strips: Vec<Vec<Vec3>> = if axes_only {
                let mut buffer: Vec<u32> = Vec::new();
                subdiv.get_major_edges_line_indices(&mut buffer, 1, |v| v.push(0));
                buffer
                    .split(|&i| i == 0)
                    .map(|strip| -> Vec<Vec3> {
                        strip
                            .iter()
                            .map(|&i| sphere_points[i as usize - 1].into())
                            .collect()
                    })
                    .collect()
            } else {
                subdiv
                    .get_all_line_indices(1, |v| v.push(0))
                    .split(|&i| i == 0)
                    .map(|strip| -> Vec<Vec3> {
                        strip
                            .iter()
                            .map(|&i| sphere_points[i as usize - 1].into())
                            .collect()
                    })
                    .collect()
            };
            WireframeMesh {
                bbox: key.simple_bounding_box(),
                vertex_count: line_strips.iter().map(|v| v.len()).sum(),
                line_strips,
            }
        }
        ProcMeshKey::Capsule {
            length,
            subdivisions,
            axes_only,
        } => {
            let line_strips = capsule_wireframe_lines(length.into_inner(), subdivisions, axes_only);

            WireframeMesh {
                bbox: key.simple_bounding_box(),
                vertex_count: line_strips.iter().map(|s| s.len()).sum(),
                line_strips,
            }
        }
        ProcMeshKey::Cylinder {
            subdivisions,
            axes_only,
        } => {
            let n = ((subdivisions + 1) * 4).max(3);
            let delta = std::f32::consts::TAU / (n as f32);
            // cylinder’s radius is 1
            let half_height = 1.0;
            let mut line_strips: Vec<Vec<Vec3>> = Vec::new();

            // bottom cap
            let mut bottom_loop = Vec::with_capacity(n + 1);
            for i in 0..n {
                let theta = i as f32 * delta;
                let x = theta.cos(); // radius = 1
                let y = theta.sin();
                bottom_loop.push(Vec3::new(x, y, -half_height));
            }

            bottom_loop.push(bottom_loop[0]);
            line_strips.push(bottom_loop);

            // top cap
            let mut top_loop = Vec::with_capacity(n + 1);
            for i in 0..n {
                let theta = i as f32 * delta;
                let x = theta.cos();
                let y = theta.sin();
                top_loop.push(Vec3::new(x, y, half_height));
            }

            // close the loop
            top_loop.push(top_loop[0]);
            line_strips.push(top_loop);

            let num_spokes = if axes_only { 6 } else { n };
            let delta = std::f32::consts::TAU / (num_spokes as f32);

            let bottom_center = Vec3::new(0.0, 0.0, -half_height);
            let top_center = Vec3::new(0.0, 0.0, half_height);

            for i in 0..num_spokes {
                let theta = (i as f32) * delta;
                let x = theta.cos();
                let y = theta.sin();

                line_strips.push(vec![
                    Vec3::new(x, y, -half_height),
                    Vec3::new(x, y, half_height),
                ]);

                // for `FillMode::DenseWireframe` we also draw a line from rim -> center
                if !axes_only {
                    // from bottom rim to bottom center
                    line_strips.push(vec![Vec3::new(x, y, -half_height), bottom_center]);

                    // from top rim to top center
                    line_strips.push(vec![Vec3::new(x, y, half_height), top_center]);
                }
            }

            WireframeMesh {
                bbox: key.simple_bounding_box(),
                vertex_count: line_strips.iter().map(|strip| strip.len()).sum(),
                line_strips,
            }
        }
    }
}

fn capsule_wireframe_lines(length: f32, subdiv: usize, axes_only: bool) -> Vec<Vec<Vec3>> {
    let mut line_strips = Vec::new();

    let n = ((subdiv + 1) * 4).max(3);
    let delta = std::f32::consts::TAU / n as f32;

    let z_top = length;
    let z_bot = 0.0;

    let mut top_loop = Vec::with_capacity(n + 1);
    let mut bottom_loop = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let theta = i as f32 * delta;

        // add top/bottom rim points
        top_loop.push(Vec3::new(theta.cos(), theta.sin(), z_top));
        bottom_loop.push(Vec3::new(theta.cos(), theta.sin(), z_bot));
    }

    line_strips.push(top_loop);
    line_strips.push(bottom_loop);

    // side spokes
    let num_spokes = if axes_only { 4 } else { n };
    let delta_spoke = std::f32::consts::TAU / num_spokes as f32;
    for i in 0..num_spokes {
        let theta = i as f32 * delta_spoke;
        let rim = Vec3::new(theta.cos(), theta.sin(), 0.0);
        line_strips.push(vec![rim.with_z(z_bot), rim.with_z(z_top)]);
    }

    // add the hemispherical caps, by taking a sphere and chopping it in two
    let sphere: Subdivided<(), OctahedronBase> = Subdivided::new(subdiv, |_| ());

    // choose which edges to emit
    let mut tmp = Vec::new();
    let indices: Vec<u32> = if axes_only {
        sphere.get_major_edges_line_indices(&mut tmp, 1, |v| v.push(0));
        tmp
    } else {
        sphere.get_all_line_indices(1, |v| v.push(0))
    };

    // we split the sphere up into strips, each strip ends in point at index 0
    // so we split the indices on index 0.
    let pts = sphere.raw_points();
    for strip in indices.split(|&i| i == 0) {
        let mut prev_top: Option<Vec3> = None;
        let mut prev_bottom: Option<Vec3> = None;

        for &idx in strip {
            let p = pts[idx as usize - 1];
            let v_top = Vec3::new(p.x, p.y, p.z + z_top);
            let v_bottom = Vec3::new(p.x, p.y, p.z);

            if let Some(p0) = prev_top {
                // connect previous top to this top
                if p0.z >= z_top && v_top.z >= z_top {
                    line_strips.push(vec![p0, v_top]);
                }
            }

            if let Some(p0) = prev_bottom {
                // connect previous bottom to this bottom
                if p0.z <= z_bot && v_bottom.z <= z_bot {
                    line_strips.push(vec![p0, v_bottom]);
                }
            }

            prev_top = Some(v_top);
            prev_bottom = Some(v_bottom);
        }
    }

    line_strips
}

// ----------------------------------------------------------------------------

/// Cache for the computation of triangle meshes from [`ProcMeshKey`]s that depict the
/// shape as a solid object.
#[derive(Default)]
pub struct SolidCache(ahash::HashMap<ProcMeshKey, Option<SolidMesh>>);

impl SolidCache {
    pub fn entry(&mut self, key: ProcMeshKey, render_ctx: &RenderContext) -> Option<SolidMesh> {
        self.0
            .entry(key)
            .or_insert_with(|| {
                re_tracing::profile_scope!("proc_mesh::SolidCache(miss)", format!("{key:?}"));

                re_log::trace!("Generating solid mesh {key:?}…");

                match generate_solid(&key, render_ctx) {
                    Ok(mesh) => Some(mesh),
                    Err(err) => {
                        re_log::warn!(
                            "Failed to generate mesh {key:?}: {}",
                            re_error::format_ref(&err)
                        );
                        None
                    }
                }
            })
            .clone()
    }
}

impl Cache for SolidCache {
    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn name(&self) -> &'static str {
        "Proc Mesh Solids"
    }

    fn vram_usage(&self) -> re_byte_size::MemUsageTree {
        let mut node = re_byte_size::MemUsageNode::new();

        let mut items: Vec<_> = self
            .0
            .iter()
            .map(|(key, mesh)| {
                let bytes_gpu = mesh.as_ref().map_or(0, |m| m.gpu_mesh.gpu_byte_size());
                (format!("{key:?}"), bytes_gpu)
            })
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        for (item_name, bytes_gpu) in items {
            node.add(item_name, re_byte_size::MemUsageTree::Bytes(bytes_gpu));
        }

        node.into_tree()
    }
}

impl re_byte_size::MemUsageTreeCapture for SolidCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}

/// Generate a solid triangle mesh without caching.
fn generate_solid(key: &ProcMeshKey, render_ctx: &RenderContext) -> Result<SolidMesh, GenError> {
    re_tracing::profile_function!();

    let bbox = key.simple_bounding_box();

    let mesh: mesh::CpuMesh = match *key {
        ProcMeshKey::Cube => {
            let mut mg = macaw::MeshGen::new();
            mg.push_cube(Vec3::splat(0.5), macaw::IsoTransform::IDENTITY);
            mesh_from_mesh_gen(format!("{key:?}").into(), mg, render_ctx, bbox)
        }
        ProcMeshKey::Sphere {
            subdivisions,
            axes_only: _, // no effect on solid mesh
        } => {
            let subdiv: hexasphere::Subdivided<(), OctahedronBase> =
                hexasphere::Subdivided::new(subdivisions, |_| ());

            let vertex_positions: Vec<Vec3> =
                subdiv.raw_points().iter().map(|&p| p.into()).collect();
            // A unit sphere's normals are its positions.
            let vertex_normals = vertex_positions.clone();
            let num_vertices = vertex_positions.len();

            let triangle_indices = subdiv.get_all_indices();
            let triangle_indices: Vec<glam::UVec3> = triangle_indices
                .into_iter()
                .tuples()
                .map(|(i1, i2, i3)| glam::uvec3(i1, i2, i3))
                .collect();

            let materials = materials_for_uncolored_mesh(render_ctx, triangle_indices.len());

            mesh::CpuMesh {
                label: format!("{key:?}").into(),

                // bytemuck is re-grouping the indices into triples without realloc
                triangle_indices,

                vertex_positions,
                vertex_normals,
                // Colors are black so that the instance `additive_tint` can set per-instance color.
                vertex_colors: vec![re_renderer::Rgba32Unmul::BLACK; num_vertices],
                vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

                materials,

                bbox,
            }
        }
        ProcMeshKey::Capsule {
            length,
            subdivisions,
            axes_only: _, // no effect on solid mesh
        } => {
            // Design note: there are two reasons why this uses `macaw` instead of `hexasphere`.
            //
            // First, `macaw` already has a capsule routine, whereas we'd have to postprocess the
            // output of `hexasphere`.
            //
            // Second, one design perspective is that we should in the long run extend `macaw`
            // to do *all* our mesh generation, and this is an experiment in that. How exactly that
            // will handle wireframes is yet undecided.

            let mg_subdivisions = (subdivisions + 1) * 4;

            let mut mg = macaw::MeshGen::new();
            mg.push_capsule(
                1.0,
                length.into_inner(),
                mg_subdivisions,
                mg_subdivisions,
                // rotate from the Y axis (baked into MeshGen) onto the Z axis (our choice of
                // default orientation, aligned with Rerun’s default of Z-up).
                macaw::IsoTransform::from_quat(glam::Quat::from_rotation_x(
                    std::f32::consts::FRAC_PI_2,
                )),
            );
            mesh_from_mesh_gen(format!("{key:?}").into(), mg, render_ctx, bbox)
        }
        ProcMeshKey::Cylinder {
            subdivisions,
            axes_only: _, // not used for solid mesh
        } => {
            let mg_subdivisions = (subdivisions + 1) * 4;

            let mut mg = macaw::MeshGen::new();

            push_cylinder_solid(&mut mg, 1.0, 2.0, mg_subdivisions);
            mesh_from_mesh_gen(format!("{key:?}").into(), mg, render_ctx, bbox)
        }
    };

    mesh.sanity_check()?;

    Ok(SolidMesh {
        bbox: key.simple_bounding_box(),
        gpu_mesh: Arc::new(GpuMesh::new(render_ctx, &mesh)?),
    })
}

/// Creates a cylinder aligned along the Y axis, centered vertically.
fn push_cylinder_solid(mesh_gen: &mut MeshGen, radius: f32, height: f32, subdivisions: usize) {
    let index_offset = mesh_gen.positions.len() as u32;
    let n = subdivisions.max(3) as u32;
    let half_height = height * 0.5;
    let delta = 2.0 * std::f32::consts::PI / n as f32;

    // top and bottom center positions
    mesh_gen.positions.push(Vec3::new(0.0, 0.0, half_height));
    mesh_gen.normals.push(Vec3::new(0.0, 0.0, 1.0));
    mesh_gen.positions.push(Vec3::new(0.0, 0.0, -half_height));
    mesh_gen.normals.push(Vec3::new(0.0, 0.0, -1.0));

    // for each slice, push rim‐and‐side vertices (4 per slice)
    for i in 0..n {
        let theta = i as f32 * delta;
        let (cos_theta, sin_theta) = (theta.cos(), theta.sin());
        let x = radius * cos_theta;
        let y = radius * sin_theta;

        // top rim point
        mesh_gen.positions.push(Vec3::new(x, y, half_height));
        mesh_gen.normals.push(Vec3::new(0.0, 0.0, 1.0));

        // bottom rim point
        mesh_gen.positions.push(Vec3::new(x, y, -half_height));
        mesh_gen.normals.push(Vec3::new(0.0, 0.0, -1.0));

        // side‐top point
        mesh_gen.positions.push(Vec3::new(x, y, half_height));
        mesh_gen.normals.push(Vec3::new(cos_theta, sin_theta, 0.0));

        // side‐bottom point
        mesh_gen.positions.push(Vec3::new(x, y, -half_height));
        mesh_gen.normals.push(Vec3::new(cos_theta, sin_theta, 0.0));
    }

    // build triangle for top and bottom caps
    for i in 0..n {
        let top_center = index_offset;
        let bot_center = index_offset + 1;
        let top_rim = index_offset + 2 + i * 4;
        let next_top = index_offset + 2 + ((i + 1) % n) * 4;

        mesh_gen
            .indices
            .extend_from_slice(&[top_center, top_rim, next_top]);

        let bot_rim = index_offset + 3 + i * 4;
        let next_bot = index_offset + 3 + ((i + 1) % n) * 4;
        mesh_gen
            .indices
            .extend_from_slice(&[bot_center, next_bot, bot_rim]);
    }

    // build side quads (split into two triangles each)
    let side_base = index_offset + 4;
    for i in 0..n {
        let top = side_base + i * 4;
        let bot = side_base + i * 4 + 1;
        let next_top = side_base + ((i + 1) % n) * 4;
        let next_bot = side_base + ((i + 1) % n) * 4 + 1;

        mesh_gen.indices.extend_from_slice(&[top, bot, next_top]);

        mesh_gen
            .indices
            .extend_from_slice(&[next_top, bot, next_bot]);
    }
}

fn mesh_from_mesh_gen(
    label: re_renderer::DebugLabel,
    mg: MeshGen,
    render_ctx: &RenderContext,
    bbox: macaw::BoundingBox,
) -> mesh::CpuMesh {
    let num_vertices = mg.positions.len();

    let triangle_indices: Vec<glam::UVec3> = mg
        .indices
        .into_iter()
        .tuples()
        .map(|(i1, i2, i3)| uvec3(i1, i2, i3))
        .collect();
    let materials = materials_for_uncolored_mesh(render_ctx, triangle_indices.len());

    mesh::CpuMesh {
        label,
        materials,
        triangle_indices,
        vertex_positions: mg.positions,
        vertex_normals: mg.normals,
        // Colors are black so that the instance `additive_tint` can set per-instance color.
        vertex_colors: vec![re_renderer::Rgba32Unmul::BLACK; num_vertices],
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],
        bbox,
    }
}

fn materials_for_uncolored_mesh(
    render_ctx: &RenderContext,
    num_triangles: usize,
) -> smallvec::SmallVec<[mesh::Material; 1]> {
    smallvec![mesh::Material {
        label: "default material".into(),
        index_range: 0..(num_triangles * 3) as u32,
        albedo: render_ctx
            .texture_manager_2d
            .white_texture_unorm_handle()
            .clone(),
        albedo_factor: re_renderer::Rgba::BLACK,
    }]
}

// ----------------------------------------------------------------------------

/// Base shape for [`hexasphere`]'s subdivision algorithm which is an octahedron
/// that is subdivided into a sphere mesh.
/// The value of this shape for us is that it has “equatorial” edges which are
/// perpendicular to the axes of the ellipsoid, which thus align with the quantities
/// the user actually specified (length on each axis), and can be usefully visualized
/// by themselves separately from the subdivision mesh.
///
/// TODO(kpreid): This would also make sense to contribute back to `hexasphere` itself.
#[derive(Clone, Copy, Debug, Default)]
struct OctahedronBase;

impl BaseShape for OctahedronBase {
    fn initial_points(&self) -> Vec<Vec3A> {
        vec![
            Vec3A::NEG_X,
            Vec3A::NEG_Y,
            Vec3A::NEG_Z,
            Vec3A::X,
            Vec3A::Y,
            Vec3A::Z,
        ]
    }

    fn triangles(&self) -> Box<[hexasphere::Triangle]> {
        use hexasphere::Triangle;
        const TRIANGLES: [Triangle; 8] = [
            Triangle::new(0, 2, 1, 1, 4, 0),   // -X-Y-Z face
            Triangle::new(0, 1, 5, 0, 6, 3),   // -X-Y+Z face
            Triangle::new(0, 4, 2, 2, 5, 1),   // -X+Y-Z face
            Triangle::new(0, 5, 4, 3, 7, 2),   // -X+Y+Z face
            Triangle::new(3, 1, 2, 8, 4, 9),   // +X-Y-Z face
            Triangle::new(3, 5, 1, 11, 6, 8),  // +X-Y+Z face
            Triangle::new(3, 2, 4, 9, 5, 10),  // +X+Y-Z face
            Triangle::new(3, 4, 5, 10, 7, 11), // +X+Y+Z face
        ];
        Box::new(TRIANGLES)
    }

    /// The octahedron has 12 edges, which we are arbitrarily numbering as follows:
    ///
    /// 0. -X to -Y
    /// 1. -X to -Z
    /// 2. -X to +Y
    /// 3. -X to +Z
    /// 4. -Z to -Y
    /// 5. -Z to +Y
    /// 6. +Z to -Y
    /// 7. +Z to +Y
    /// 8. +X to -Y
    /// 9. +X to -Z
    /// 10. +X to +Y
    /// 11. +X to +Z
    const EDGES: usize = 12;

    fn interpolate(&self, a: Vec3A, b: Vec3A, p: f32) -> Vec3A {
        hexasphere::interpolation::geometric_slerp(a, b, p)
    }
}
