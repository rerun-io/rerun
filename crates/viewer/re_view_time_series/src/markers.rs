//! GPU mesh definitions for [`MarkerShape`] markers.
//!
//! Each marker shape is encoded as a flat 2D triangle mesh in unit-radius local space.
//! A single mesh per shape is uploaded once and reused for every marker via instancing —
//! per-instance transforms position/scale the marker, and the per-instance `additive_tint`
//! colors it.

use std::sync::Arc;

use glam::{Vec2, Vec3, vec2, vec3};
use re_renderer::{
    Color32, OutlineMaskPreference, PickingLayerId, RenderContext, ShapeBuilder,
    mesh::{CpuMesh, GpuMesh, MeshError},
    renderer::GpuMeshInstance,
};
use re_sdk_types::components::MarkerShape;
use re_sdk_types::reflection::Enum as _;
use re_viewer_context::Cache;

/// Multiplier applied to the marker radius when the series is highlighted (hovered/selected).
///
/// Matches `egui_plot`'s highlight behavior: the marker area doubles, so the radius scales by √2.
pub const HIGHLIGHT_RADIUS_EXPANSION: f32 = 1.0;

/// Triangle-fan resolution for the [`MarkerShape::Circle`] mesh.
const CIRCLE_SEGMENTS: usize = 32;

/// Stroke width for open shapes (Cross/Plus/Asterisk), as a fraction of the unit radius.
/// Matches `egui_plot`'s default of `radius / 5`.
const STROKE_HALF_WIDTH: f32 = 0.1;

/// One [`GpuMesh`] per [`MarkerShape`] variant, indexed by `shape as usize - 1`
/// (the discriminants are `repr(u8)` and 1-based; see `marker_shape.rs`).
pub struct MarkerMeshes {
    meshes: Vec<Arc<GpuMesh>>,
}

impl MarkerMeshes {
    fn build(render_ctx: &RenderContext) -> Result<Self, MeshError> {
        let meshes = MarkerShape::variants()
            .iter()
            .map(|&shape| {
                let cpu_mesh = build_marker_cpu_mesh(shape, render_ctx);
                GpuMesh::new(render_ctx, &cpu_mesh).map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { meshes })
    }

    pub fn for_shape(&self, shape: MarkerShape) -> Arc<GpuMesh> {
        // `MarkerShape` is `repr(u8)` with 1-based discriminants.
        self.meshes[shape as usize - 1].clone()
    }
}

/// Per-recording cache that lazily builds and stores [`MarkerMeshes`].
///
/// Marker meshes are tiny and shape-only — they don't depend on per-view state — so a
/// single set is shared across every [`crate::TimeSeriesView`] instance in a recording.
#[derive(Default)]
pub struct MarkerMeshCache {
    meshes: Option<Arc<MarkerMeshes>>,

    /// Set once if the initial upload fails, so we don't spam logs on every frame.
    build_failed: bool,
}

impl MarkerMeshCache {
    /// Returns the cached meshes, building them on first call.
    /// Returns `None` (and logs once) if the upload ever fails.
    pub fn get_or_build(&mut self, render_ctx: &RenderContext) -> Option<Arc<MarkerMeshes>> {
        if let Some(meshes) = &self.meshes {
            return Some(meshes.clone());
        }
        if self.build_failed {
            return None;
        }
        match MarkerMeshes::build(render_ctx) {
            Ok(meshes) => {
                let arc = Arc::new(meshes);
                self.meshes = Some(arc.clone());
                Some(arc)
            }
            Err(err) => {
                re_log::error_once!("Failed to upload marker meshes: {err}");
                self.build_failed = true;
                None
            }
        }
    }
}

impl Cache for MarkerMeshCache {
    fn name(&self) -> &'static str {
        "MarkerMeshCache"
    }

    fn purge_memory(&mut self) {
        self.meshes = None;
        self.build_failed = false;
    }

    fn vram_usage(&self) -> re_byte_size::MemUsageTree {
        let bytes = self
            .meshes
            .as_ref()
            .map_or(0, |m| m.meshes.iter().map(|gpu| gpu.gpu_byte_size()).sum());
        re_byte_size::MemUsageTree::Bytes(bytes)
    }
}

impl re_byte_size::SizeBytes for MarkerMeshCache {
    fn heap_size_bytes(&self) -> u64 {
        self.meshes.as_ref().map_or(0, |m| {
            (m.meshes.capacity() * std::mem::size_of::<Arc<GpuMesh>>()) as u64
        })
    }
}

impl re_byte_size::MemUsageTreeCapture for MarkerMeshCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(<Self as re_byte_size::SizeBytes>::total_size_bytes(self))
    }
}

/// World-space z for marker mesh instances.
///
/// Lines sit at world z = 0. With `OrthographicCameraMode::TopLeftCornerAndExtendZ`,
/// `re_renderer` negates the world-z axis when going to view space (`view_builder.rs`),
/// and the projection then maps positive view-z to NDC z = 1. Combined with the
/// `GreaterEqual` depth test, that means *negative* world z is closer to camera.
///
/// We pick a small negative value so markers draw on top of lines: their NDC z is
/// strictly greater than the line's, so the line's pixels fail the depth test wherever
/// a marker covers them.
const MARKER_DEPTH: f32 = -1.0;

/// Build a single [`GpuMeshInstance`] for a marker of the given shape, centered at `center`,
/// scaled by `radius`, and tinted with `color`.
pub fn marker_instance(
    mesh: Arc<GpuMesh>,
    center: Vec2,
    radius: f32,
    color: Color32,
) -> GpuMeshInstance {
    GpuMeshInstance {
        gpu_mesh: mesh,
        world_from_mesh: glam::Affine3A::from_scale_rotation_translation(
            Vec3::splat(radius),
            glam::Quat::IDENTITY,
            vec3(center.x, center.y, MARKER_DEPTH),
        ),
        additive_tint: color,
        outline_mask_ids: OutlineMaskPreference::NONE,
        picking_layer_id: PickingLayerId::default(),
        cull_mode: None,
    }
}

/// Build a flat 2D unit-radius mesh for the given marker shape.
fn build_marker_cpu_mesh(shape: MarkerShape, render_ctx: &RenderContext) -> CpuMesh {
    let label = format!("marker_{shape:?}");
    let mut builder = ShapeBuilder::default();

    let sqrt_3 = 3_f32.sqrt();
    let half_sqrt_3 = sqrt_3 * 0.5;
    let inv_sqrt_2 = 1.0 / 2_f32.sqrt();

    match shape {
        MarkerShape::Circle => {
            builder.add_triangle_fan(
                Vec2::ZERO,
                |i| {
                    let theta = std::f32::consts::TAU * (i as f32) / (CIRCLE_SEGMENTS as f32);
                    vec2(theta.cos(), theta.sin())
                },
                CIRCLE_SEGMENTS,
            );
        }
        MarkerShape::Diamond => {
            builder.add_convex_polygon(&[
                vec2(0.0, 1.0),
                vec2(-1.0, 0.0),
                vec2(0.0, -1.0),
                vec2(1.0, 0.0),
            ]);
        }
        MarkerShape::Square => {
            builder.add_convex_polygon(&[
                vec2(-inv_sqrt_2, inv_sqrt_2),
                vec2(-inv_sqrt_2, -inv_sqrt_2),
                vec2(inv_sqrt_2, -inv_sqrt_2),
                vec2(inv_sqrt_2, inv_sqrt_2),
            ]);
        }
        MarkerShape::Up => {
            builder.add_convex_polygon(&[
                vec2(0.0, -1.0),
                vec2(half_sqrt_3, 0.5),
                vec2(-half_sqrt_3, 0.5),
            ]);
        }
        MarkerShape::Down => {
            builder.add_convex_polygon(&[
                vec2(0.0, 1.0),
                vec2(-half_sqrt_3, -0.5),
                vec2(half_sqrt_3, -0.5),
            ]);
        }
        MarkerShape::Left => {
            builder.add_convex_polygon(&[
                vec2(-1.0, 0.0),
                vec2(0.5, -half_sqrt_3),
                vec2(0.5, half_sqrt_3),
            ]);
        }
        MarkerShape::Right => {
            builder.add_convex_polygon(&[
                vec2(1.0, 0.0),
                vec2(-0.5, half_sqrt_3),
                vec2(-0.5, -half_sqrt_3),
            ]);
        }
        MarkerShape::Cross => {
            builder.add_segment(
                vec2(-inv_sqrt_2, -inv_sqrt_2),
                vec2(inv_sqrt_2, inv_sqrt_2),
                STROKE_HALF_WIDTH,
            );
            builder.add_segment(
                vec2(inv_sqrt_2, -inv_sqrt_2),
                vec2(-inv_sqrt_2, inv_sqrt_2),
                STROKE_HALF_WIDTH,
            );
        }
        MarkerShape::Plus => {
            builder.add_segment(vec2(-1.0, 0.0), vec2(1.0, 0.0), STROKE_HALF_WIDTH);
            builder.add_segment(vec2(0.0, -1.0), vec2(0.0, 1.0), STROKE_HALF_WIDTH);
        }
        MarkerShape::Asterisk => {
            builder.add_segment(vec2(0.0, -1.0), vec2(0.0, 1.0), STROKE_HALF_WIDTH);
            builder.add_segment(
                vec2(-half_sqrt_3, 0.5),
                vec2(half_sqrt_3, -0.5),
                STROKE_HALF_WIDTH,
            );
            builder.add_segment(
                vec2(-half_sqrt_3, -0.5),
                vec2(half_sqrt_3, 0.5),
                STROKE_HALF_WIDTH,
            );
        }
    }

    builder.into_cpu_mesh(label, render_ctx)
}
