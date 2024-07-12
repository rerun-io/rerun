use std::sync::Arc;

use ahash::HashSet;
use glam::Vec3;

use re_renderer::RenderContext;
use re_viewer_context::Cache;

// ----------------------------------------------------------------------------

/// Description of a mesh that can be procedurally generated.
///
/// Obtain the actual mesh by passing this to [`WireframeCache`].
/// In the future, it will be possible to produce solid triangle meshes too.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ProcMeshKey {
    /// A sphere of unit radius.
    Sphere { subdivisions: usize },
}

/// A renderable mesh generated from a [`ProcMeshKey`] by the [`WireframeCache`],
/// which is to be drawn as lines rather than triangles.
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
    // In the future, render_ctx will be used to allocate GPU memory for the mesh.
    _ = render_ctx;

    match *key {
        ProcMeshKey::Sphere { subdivisions } => {
            let subdiv = hexasphere::shapes::IcoSphere::new(subdivisions, |other_glam_vec| {
                // `hexasphere` uses a different version of `glam` than we do.
                // <https://github.com/OptimisticPeach/hexasphere/issues/19>
                <[f32; 3]>::from(other_glam_vec).into()
            });

            let sphere_points = subdiv.raw_data();

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
                            .map(|&i| sphere_points[i as usize - 1])
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
                    .map(|(i1, i2)| vec![sphere_points[i1 as usize], sphere_points[i2 as usize]])
                    .collect()
            };
            WireframeMesh {
                bbox: re_math::BoundingBox::from_center_size(Vec3::splat(0.0), Vec3::splat(1.0)),
                vertex_count: line_strips.iter().map(|v| v.len()).sum(),
                line_strips,
            }
        }
    }
}

// TODO(kpreid): A solid (rather than wireframe) mesh cache implementation should live here.
