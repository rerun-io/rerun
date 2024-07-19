//! Procedurally-generated meshes for rendering objects that are
//! specified geometrically, and have nontrivial numbers of vertices each,
//! such as a sphere or cylinder.

use std::sync::Arc;

use ahash::HashSet;
use glam::{uvec3, vec3, Vec3};
use itertools::Itertools as _;
use smallvec::smallvec;

use re_renderer::mesh;
use re_renderer::resource_managers::{GpuMeshHandle, ResourceManagerError};
use re_renderer::RenderContext;
use re_viewer_context::Cache;

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
    Sphere { subdivisions: usize },
}

impl ProcMeshKey {
    /// Returns the bounding box which can be computed from the mathematical shape,
    /// without regard for its exact approximation as a mesh.
    fn simple_bounding_box(&self) -> re_math::BoundingBox {
        match self {
            Self::Sphere { subdivisions: _ } => {
                // sphere’s radius is 1, so its size is 2
                re_math::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(2.0))
            }
            Self::Cube => {
                re_math::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(1.0))
            }
        }
    }
}

/// A renderable mesh generated from a [`ProcMeshKey`] by the [`WireframeCache`],
/// which is to be drawn as lines rather than triangles.
#[derive(Debug)]
pub struct WireframeMesh {
    pub bbox: re_math::BoundingBox,

    pub vertex_count: usize,

    /// Collection of line strips making up the wireframe.
    ///
    /// TODO(kpreid): This should instead be a GPU buffer, but we don’t yet have a
    /// `re_renderer::Renderer` implementation that takes instanced meshes and applies
    /// the line shader to them, instead of doing immediate-mode accumulation of line strips.
    pub line_strips: Vec<Vec<Vec3>>,
}

/// A renderable mesh generated from a [`ProcMeshKey`] by the [`SolidCache`],
/// which is to be drawn as triangles rather than lines.
///
/// This type is cheap to clone.
#[derive(Debug, Clone)]
pub struct SolidMesh {
    pub bbox: re_math::BoundingBox,

    /// Mesh to render. Note that its colors are set to black, so that the
    /// `MeshInstance::additive_tint` can be used to set the color per instance.
    pub gpu_mesh: GpuMeshHandle,
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
        re_tracing::profile_function!();

        self.0
            .entry(key)
            .or_insert_with(|| {
                re_log::debug!("Generating mesh {key:?}…");

                let mesh = generate_wireframe(&key, render_ctx);

                // Right now, this can never return None, but in the future
                // it will perform GPU allocations which can fail.

                Some(Arc::new(mesh))
            })
            .clone()
    }
}

impl Cache for WireframeCache {
    fn begin_frame(&mut self) {}

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Generate a wireframe mesh without caching.
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
        ProcMeshKey::Sphere { subdivisions } => {
            let subdiv = hexasphere::shapes::IcoSphere::new(subdivisions, |_| ());

            let sphere_points = subdiv.raw_points();

            // TODO(kpreid): There is a bug in `hexasphere` where it fails to return lines which
            // reach the original corners of the shape. This will be fixed as part of
            // <https://github.com/OptimisticPeach/hexasphere/issues/19>,
            // which is merged but not yet published on crates.io.
            // When hexasphere 15.0 or 14.0.1 is available, update, then keep the first branch
            // of this `if` only.
            let line_strips: Vec<Vec<Vec3>> = if false {
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
            } else {
                // Gather edges from the triangles, deduplicating.
                let lines: HashSet<(u32, u32)> = subdiv
                    .get_all_indices()
                    .chunks(3)
                    .flat_map(|triangle| {
                        let [i1, i2, i3] = <[u32; 3]>::try_from(triangle).unwrap();
                        [(i1, i2), (i2, i3), (i3, i1)]
                    })
                    .map(|(i1, i2)| if i1 > i2 { (i2, i1) } else { (i1, i2) })
                    .collect();

                lines
                    .into_iter()
                    .map(|(i1, i2)| {
                        vec![
                            sphere_points[i1 as usize].into(),
                            sphere_points[i2 as usize].into(),
                        ]
                    })
                    .collect()
            };
            WireframeMesh {
                bbox: key.simple_bounding_box(),
                vertex_count: line_strips.iter().map(|v| v.len()).sum(),
                line_strips,
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Cache for the computation of triangle meshes from [`ProcMeshKey`]s that depict the
/// shape as a solid object.
#[derive(Default)]
pub struct SolidCache(ahash::HashMap<ProcMeshKey, Option<SolidMesh>>);

impl SolidCache {
    pub fn entry(&mut self, key: ProcMeshKey, render_ctx: &RenderContext) -> Option<SolidMesh> {
        re_tracing::profile_function!();

        self.0
            .entry(key)
            .or_insert_with(|| {
                re_log::debug!("Generating mesh {key:?}…");

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
    fn begin_frame(&mut self) {}

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Generate a solid triangle mesh without caching.
fn generate_solid(
    key: &ProcMeshKey,
    render_ctx: &RenderContext,
) -> Result<SolidMesh, ResourceManagerError> {
    re_tracing::profile_function!();

    let mesh: mesh::Mesh = match *key {
        ProcMeshKey::Cube => {
            let mut mg = re_math::MeshGen::new();
            mg.push_cube(Vec3::splat(0.5), re_math::IsoTransform::IDENTITY);

            let num_vertices = mg.positions.len();

            let triangle_indices: Vec<glam::UVec3> = mg
                .indices
                .into_iter()
                .tuples()
                .map(|(i1, i2, i3)| uvec3(i1, i2, i3))
                .collect();
            let materials = materials_for_uncolored_mesh(render_ctx, triangle_indices.len());

            mesh::Mesh {
                label: format!("{key:?}").into(),
                materials,
                triangle_indices,
                vertex_positions: mg.positions,
                vertex_normals: mg.normals,
                // Colors are black so that the instance `additive_tint` can set per-instance color.
                vertex_colors: vec![re_renderer::Rgba32Unmul::BLACK; num_vertices],
                vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],
            }
        }
        ProcMeshKey::Sphere { subdivisions } => {
            let subdiv = hexasphere::shapes::IcoSphere::new(subdivisions, |_| ());

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

            mesh::Mesh {
                label: format!("{key:?}").into(),

                // bytemuck is re-grouping the indices into triples without realloc
                triangle_indices,

                vertex_positions,
                vertex_normals,
                // Colors are black so that the instance `additive_tint` can set per-instance color.
                vertex_colors: vec![re_renderer::Rgba32Unmul::BLACK; num_vertices],
                vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

                materials,
            }
        }
    };

    mesh.sanity_check()?;

    let gpu_mesh = render_ctx.mesh_manager.write().create(
        render_ctx,
        &mesh,
        re_renderer::resource_managers::ResourceLifeTime::LongLived,
    )?;

    Ok(SolidMesh {
        bbox: key.simple_bounding_box(),
        gpu_mesh,
    })
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
