//! A small builder for flat 2D [`CpuMesh`]es composed of convex polygons, triangle fans,
//! and stroked segments — useful for marker shapes and similar 2D primitives.

use glam::{Vec2, Vec3, vec2, vec3};
use smallvec::smallvec;

use crate::{
    Rgba, Rgba32Unmul,
    context::RenderContext,
    mesh::{CpuMesh, Material},
};

#[derive(Default)]
pub struct ShapeBuilder {
    positions: Vec<Vec3>,
    indices: Vec<glam::UVec3>,
}

impl ShapeBuilder {
    pub fn add_convex_polygon(&mut self, points: &[Vec2]) {
        re_log::debug_assert!(points.len() >= 3);
        let base = self.positions.len() as u32;
        for &p in points {
            self.positions.push(vec3(p.x, p.y, 0.0));
        }
        for i in 1..(points.len() as u32 - 1) {
            self.indices.push(glam::uvec3(base, base + i, base + i + 1));
        }
    }

    pub fn add_triangle_fan(
        &mut self,
        center: Vec2,
        ring: impl Fn(usize) -> Vec2,
        segments: usize,
    ) {
        let base = self.positions.len() as u32;
        self.positions.push(vec3(center.x, center.y, 0.0));
        for i in 0..segments {
            let p = ring(i);
            self.positions.push(vec3(p.x, p.y, 0.0));
        }
        for i in 0..segments as u32 {
            let next = (i + 1) % segments as u32;
            self.indices
                .push(glam::uvec3(base, base + 1 + i, base + 1 + next));
        }
    }

    /// Add a stroked line segment as a quad (two triangles) with the given half-width.
    pub fn add_segment(&mut self, a: Vec2, b: Vec2, half_width: f32) {
        let dir = (b - a).normalize_or_zero();
        let normal = vec2(-dir.y, dir.x) * half_width;
        let p0 = a + normal;
        let p1 = b + normal;
        let p2 = b - normal;
        let p3 = a - normal;
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&[
            vec3(p0.x, p0.y, 0.0),
            vec3(p1.x, p1.y, 0.0),
            vec3(p2.x, p2.y, 0.0),
            vec3(p3.x, p3.y, 0.0),
        ]);
        self.indices.push(glam::uvec3(base, base + 1, base + 2));
        self.indices.push(glam::uvec3(base, base + 2, base + 3));
    }

    pub fn into_cpu_mesh(self, label: String, render_ctx: &RenderContext) -> CpuMesh {
        let Self { positions, indices } = self;
        let num_vertices = positions.len();
        // The index buffer in `CpuMesh` is `Vec<UVec3>` (one entry per triangle), but
        // `Material::index_range` is in units of scalar u32 indices, hence ×3.
        let index_count = (indices.len() * 3) as u32;
        // Unit-radius shapes are bounded by [-1, 1] in xy. Give the bbox a tiny z extent so
        // it doesn't fail `BoundingBox::is_nothing`.
        let bbox = macaw::BoundingBox::from_min_max(vec3(-1.0, -1.0, 0.0), vec3(1.0, 1.0, 0.0));
        let albedo = render_ctx
            .texture_manager_2d
            .white_texture_unorm_handle()
            .clone();
        CpuMesh {
            label: label.clone().into(),
            triangle_indices: indices,
            vertex_positions: positions,
            vertex_colors: vec![Rgba32Unmul::WHITE; num_vertices],
            vertex_normals: vec![vec3(0.0, 0.0, 1.0); num_vertices],
            vertex_texcoords: vec![Vec2::ZERO; num_vertices],
            // albedo_factor = BLACK = (0,0,0,1) so the per-instance `additive_tint` becomes
            // the full marker color (see `instanced_mesh.wgsl` for the exact formula).
            materials: smallvec![Material {
                label: label.into(),
                index_range: 0..index_count,
                albedo,
                albedo_factor: Rgba::BLACK,
            }],
            bbox,
        }
    }
}
