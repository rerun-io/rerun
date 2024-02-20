use std::ops::Range;

use re_log::ResultExt;

use crate::{
    allocator::{CpuWriteGpuReadError, DataTextureSource},
    renderer::{
        gpu_data::{LineStripInfo, LineVertex},
        LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags,
    },
    Color32, DebugLabel, DepthOffset, OutlineMaskPreference, PickingLayerInstanceId,
    PickingLayerObjectId, RenderContext, Size,
};

/// Builder for a vector of line strips, making it easy to create [`crate::renderer::LineDrawData`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location for all its data.
pub struct LineDrawableBuilder<'ctx> {
    pub ctx: &'ctx RenderContext,

    pub(crate) vertices: Vec<LineVertex>,
    pub(crate) batches: Vec<LineBatchInfo>,
    pub(crate) strips_buffer: DataTextureSource<'ctx, LineStripInfo>,

    /// Buffer for picking instance id - every strip gets its own instance id.
    /// Therefore, there need to be always as many picking instance ids as there are strips.
    pub(crate) picking_instance_ids_buffer: DataTextureSource<'ctx, PickingLayerInstanceId>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl<'ctx> LineDrawableBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            vertices: Vec::new(),
            strips_buffer: DataTextureSource::new(ctx),
            batches: Vec::with_capacity(16),
            picking_instance_ids_buffer: DataTextureSource::new(ctx),
            radius_boost_in_ui_points_for_outlines: 0.0,
        }
    }

    /// Returns number of strips that can be added without reallocation.
    /// This may be smaller than the requested number if the maximum number of strips is reached.
    pub fn reserve_strips(&mut self, num_strips: usize) -> Result<usize, CpuWriteGpuReadError> {
        // We know that the maximum number is independent of datatype, so we can use the same value for all.
        self.strips_buffer.reserve(num_strips)?;
        self.picking_instance_ids_buffer.reserve(num_strips)
    }

    /// Returns number of vertices that can be added without reallocation.
    /// This may be smaller than the requested number if the maximum number of vertices is reached.
    #[allow(clippy::unnecessary_wraps)] // TODO(andreas): will be needed once vertices writes directly to GPU readable memory.
    pub fn reserve_vertices(&mut self, num_vertices: usize) -> Result<usize, CpuWriteGpuReadError> {
        self.vertices.reserve(num_vertices);
        Ok(self.vertices.capacity() - self.vertices.len())
    }

    /// Boosts the size of the points by the given amount of ui-points for the purpose of drawing outlines.
    pub fn radius_boost_in_ui_points_for_outlines(
        &mut self,
        radius_boost_in_ui_points_for_outlines: f32,
    ) {
        self.radius_boost_in_ui_points_for_outlines = radius_boost_in_ui_points_for_outlines;
    }

    /// Start of a new batch.
    pub fn batch(&mut self, label: impl Into<DebugLabel>) -> LineBatchBuilder<'_, 'ctx> {
        self.batches.push(LineBatchInfo {
            label: label.into(),
            ..LineBatchInfo::default()
        });

        LineBatchBuilder(self)
    }

    // Iterate over all batches, yielding the batch info and all line vertices (note that these will span several line strips!)
    pub fn iter_vertices_by_batch(
        &self,
    ) -> impl Iterator<Item = (&LineBatchInfo, impl Iterator<Item = &LineVertex>)> {
        let mut vertex_offset = 0;
        self.batches.iter().map(move |batch| {
            let out = (
                batch,
                self.vertices
                    .iter()
                    .skip(vertex_offset)
                    .take(batch.line_vertex_count as usize),
            );
            vertex_offset += batch.line_vertex_count as usize;
            out
        })
    }

    /// Finalizes the builder and returns a line draw data with all the lines added so far.
    pub fn into_draw_data(self) -> Result<LineDrawData, LineDrawDataError> {
        LineDrawData::new(self)
    }

    pub fn is_empty(&self) -> bool {
        self.strips_buffer.is_empty()
    }

    pub fn default_box_flags() -> LineStripFlags {
        LineStripFlags::FLAG_CAP_END_ROUND
            | LineStripFlags::FLAG_CAP_START_ROUND
            | LineStripFlags::FLAG_CAP_END_EXTEND_OUTWARDS
            | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS
    }
}

pub struct LineBatchBuilder<'a, 'ctx>(&'a mut LineDrawableBuilder<'ctx>);

impl<'a, 'ctx> Drop for LineBatchBuilder<'a, 'ctx> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().line_vertex_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl<'a, 'ctx> LineBatchBuilder<'a, 'ctx> {
    #[inline]
    fn batch_mut(&mut self) -> &mut LineBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on PointCloudBatchBuilder creation")
    }

    fn add_vertices(&mut self, points: impl Iterator<Item = glam::Vec3>, strip_index: u32) {
        let old_len = self.0.vertices.len();

        self.0.vertices.extend(points.map(|pos| LineVertex {
            position: pos,
            strip_index,
        }));
        self.batch_mut().line_vertex_count += (self.0.vertices.len() - old_len) as u32;
    }

    /// Sets the `world_from_obj` matrix for the *entire* batch.
    #[inline]
    pub fn world_from_obj(mut self, world_from_obj: glam::Affine3A) -> Self {
        self.batch_mut().world_from_obj = world_from_obj;
        self
    }

    /// Sets an outline mask for every element in the batch.
    #[inline]
    pub fn outline_mask_ids(mut self, outline_mask_ids: OutlineMaskPreference) -> Self {
        self.batch_mut().overall_outline_mask_ids = outline_mask_ids;
        self
    }

    /// Sets the picking object id for every element in the batch.
    #[inline]
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    /// Sets the depth offset for the entire batch.
    #[inline]
    pub fn depth_offset(mut self, depth_offset: DepthOffset) -> Self {
        self.batch_mut().depth_offset = depth_offset;
        self
    }

    /// Sets the length factor as multiple of a line's radius applied to all triangle caps in this batch.
    ///
    /// This controls how far the "pointy end" of the triangle/arrow-head extends.
    /// (defaults to 4.0)
    #[inline]
    pub fn triangle_cap_length_factor(mut self, triangle_cap_length_factor: f32) -> Self {
        self.batch_mut().triangle_cap_length_factor = triangle_cap_length_factor;
        self
    }

    /// Sets the width factor as multiple of a line's radius applied to all triangle caps in this batch.
    ///
    /// This controls how wide the triangle/arrow-head is orthogonal to the line's direction.
    /// (defaults to 2.0)
    #[inline]
    pub fn triangle_cap_width_factor(mut self, triangle_cap_width_factor: f32) -> Self {
        self.batch_mut().triangle_cap_width_factor = triangle_cap_width_factor;
        self
    }

    /// Adds a 3D series of line connected points.
    pub fn add_strip(
        &mut self,
        points: impl Iterator<Item = glam::Vec3>,
    ) -> LineStripBuilder<'_, 'ctx> {
        let old_strip_count = self.0.strips_buffer.len();
        let old_vertex_count = self.0.vertices.len();
        let strip_index = old_strip_count as _;

        self.add_vertices(points, strip_index);
        let new_vertex_count = self.0.vertices.len();

        // TODO: check for max strips

        LineStripBuilder {
            builder: self.0,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_instance_id: PickingLayerInstanceId::default(),
            vertex_range: old_vertex_count..new_vertex_count,
            num_strips_added: 1,
            strip: LineStripInfo::default(),
        }
    }

    /// Adds a single 3D line segment connecting two points.
    #[inline]
    pub fn add_segment(&mut self, a: glam::Vec3, b: glam::Vec3) -> LineStripBuilder<'_, 'ctx> {
        self.add_strip([a, b].into_iter())
    }

    /// Adds a series of unconnected 3D line segments.
    pub fn add_segments(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec3, glam::Vec3)>,
    ) -> LineStripBuilder<'_, 'ctx> {
        #![allow(clippy::tuple_array_conversions)] // false positive

        debug_assert_eq!(
            self.0.strips_buffer.len(),
            self.0.picking_instance_ids_buffer.len()
        );

        let old_strip_count = self.0.strips_buffer.len();
        let old_vertex_count = self.0.vertices.len();
        let mut strip_index = old_strip_count as u32;

        // It's tempting to assign the same strip to all vertices, after all they share
        // color/radius/tag properties.
        // However, if we don't assign different strip indices, we don't know when a strip (==segment) starts and ends.
        for (a, b) in segments {
            self.add_vertices([a, b].into_iter(), strip_index);
            strip_index += 1;
        }
        let new_vertex_count = self.0.vertices.len();
        let num_strips_added = strip_index as usize - old_strip_count;

        // TODO: check for max strips

        LineStripBuilder {
            builder: self.0,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_instance_id: PickingLayerInstanceId::default(),
            vertex_range: old_vertex_count..new_vertex_count,
            num_strips_added,
            strip: LineStripInfo::default(),
        }
    }

    /// Add box outlines from a unit cube transformed by `transform`.
    ///
    /// Internally adds 12 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_box_outline_from_transform(
        &mut self,
        transform: glam::Affine3A,
    ) -> LineStripBuilder<'_, 'ctx> {
        let corners = [
            transform.transform_point3(glam::vec3(-0.5, -0.5, -0.5)),
            transform.transform_point3(glam::vec3(-0.5, -0.5, 0.5)),
            transform.transform_point3(glam::vec3(-0.5, 0.5, -0.5)),
            transform.transform_point3(glam::vec3(-0.5, 0.5, 0.5)),
            transform.transform_point3(glam::vec3(0.5, -0.5, -0.5)),
            transform.transform_point3(glam::vec3(0.5, -0.5, 0.5)),
            transform.transform_point3(glam::vec3(0.5, 0.5, -0.5)),
            transform.transform_point3(glam::vec3(0.5, 0.5, 0.5)),
        ];
        self.add_segments(
            [
                // bottom:
                (corners[0b000], corners[0b001]),
                (corners[0b000], corners[0b010]),
                (corners[0b011], corners[0b001]),
                (corners[0b011], corners[0b010]),
                // top:
                (corners[0b100], corners[0b101]),
                (corners[0b100], corners[0b110]),
                (corners[0b111], corners[0b101]),
                (corners[0b111], corners[0b110]),
                // sides:
                (corners[0b000], corners[0b100]),
                (corners[0b001], corners[0b101]),
                (corners[0b010], corners[0b110]),
                (corners[0b011], corners[0b111]),
            ]
            .into_iter(),
        )
        .flags(LineDrawableBuilder::default_box_flags())
    }

    /// Add box outlines.
    ///
    /// Internally adds 12 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    ///
    /// Returns None for empty and non-finite boxes.
    #[inline]
    pub fn add_box_outline(
        &mut self,
        bbox: &macaw::BoundingBox,
    ) -> Option<LineStripBuilder<'_, 'ctx>> {
        if !bbox.is_something() || !bbox.is_finite() {
            return None;
        }

        let corners = bbox.corners();
        Some(
            self.add_segments(
                [
                    // bottom:
                    (corners[0b000], corners[0b001]),
                    (corners[0b000], corners[0b010]),
                    (corners[0b011], corners[0b001]),
                    (corners[0b011], corners[0b010]),
                    // top:
                    (corners[0b100], corners[0b101]),
                    (corners[0b100], corners[0b110]),
                    (corners[0b111], corners[0b101]),
                    (corners[0b111], corners[0b110]),
                    // sides:
                    (corners[0b000], corners[0b100]),
                    (corners[0b001], corners[0b101]),
                    (corners[0b010], corners[0b110]),
                    (corners[0b011], corners[0b111]),
                ]
                .into_iter(),
            )
            .flags(LineDrawableBuilder::default_box_flags()),
        )
    }

    /// Add rectangle outlines.
    ///
    /// Internally adds 4 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_rectangle_outline(
        &mut self,
        top_left_corner: glam::Vec3,
        extent_u: glam::Vec3,
        extent_v: glam::Vec3,
    ) -> LineStripBuilder<'_, 'ctx> {
        self.add_segments(
            [
                (top_left_corner, top_left_corner + extent_u),
                (
                    top_left_corner + extent_u,
                    top_left_corner + extent_u + extent_v,
                ),
                (
                    top_left_corner + extent_u + extent_v,
                    top_left_corner + extent_v,
                ),
                (top_left_corner + extent_v, top_left_corner),
            ]
            .into_iter(),
        )
        .flags(LineDrawableBuilder::default_box_flags())
    }

    /// Adds a 2D series of line connected points.
    ///
    /// Uses autogenerated depth value.
    #[inline]
    pub fn add_strip_2d(
        &mut self,
        points: impl Iterator<Item = glam::Vec2>,
    ) -> LineStripBuilder<'_, 'ctx> {
        self.add_strip(points.map(|p| p.extend(0.0)))
            .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }

    /// Adds a single 2D line segment connecting two points. Uses autogenerated depth value.
    #[inline]
    pub fn add_segment_2d(&mut self, a: glam::Vec2, b: glam::Vec2) -> LineStripBuilder<'_, 'ctx> {
        self.add_strip_2d([a, b].into_iter())
            .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }

    /// Adds a series of unconnected 2D line segments.
    ///
    /// Uses autogenerated depth value, all segments get the same depth value.
    #[inline]
    pub fn add_segments_2d(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec2, glam::Vec2)>,
    ) -> LineStripBuilder<'_, 'ctx> {
        self.add_segments(segments.map(|(a, b)| (a.extend(0.0), b.extend(0.0))))
            .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }

    /// Add 2D rectangle outlines.
    ///
    /// Internally adds 4 2D line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_rectangle_outline_2d(
        &mut self,
        top_left_corner: glam::Vec2,
        extent_u: glam::Vec2,
        extent_v: glam::Vec2,
    ) -> LineStripBuilder<'_, 'ctx> {
        self.add_rectangle_outline(
            top_left_corner.extend(0.0),
            extent_u.extend(0.0),
            extent_v.extend(0.0),
        )
        .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }

    /// Add 2D rectangle outlines with axis along X and Y.
    ///
    /// Internally adds 4 2D line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_axis_aligned_rectangle_outline_2d(
        &mut self,
        min: glam::Vec2,
        max: glam::Vec2,
    ) -> LineStripBuilder<'_, 'ctx> {
        self.add_rectangle_outline(
            min.extend(0.0),
            glam::Vec3::X * (max.x - min.x),
            glam::Vec3::Y * (max.y - min.y),
        )
        .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }
}

pub struct LineStripBuilder<'a, 'ctx> {
    builder: &'a mut LineDrawableBuilder<'ctx>,
    outline_mask_ids: OutlineMaskPreference,
    vertex_range: Range<usize>,

    picking_instance_id: PickingLayerInstanceId,
    strip: LineStripInfo,
    num_strips_added: usize,
}

impl<'a, 'ctx> LineStripBuilder<'a, 'ctx> {
    #[inline]
    pub fn radius(mut self, radius: Size) -> Self {
        self.strip.radius = radius.into();
        self
    }

    #[inline]
    pub fn color(mut self, color: Color32) -> Self {
        self.strip.color = color;
        self
    }

    /// Adds (!) flags to the line strip.
    #[inline]
    pub fn flags(mut self, flags: LineStripFlags) -> Self {
        self.strip.flags |= flags;
        self
    }

    #[inline]
    pub fn picking_instance_id(mut self, instance_id: PickingLayerInstanceId) -> Self {
        self.picking_instance_id = instance_id;
        self
    }

    /// Sets an individual outline mask ids.
    /// Note that this has a relatively high performance impact.
    #[inline]
    pub fn outline_mask_ids(mut self, outline_mask_ids: OutlineMaskPreference) -> Self {
        self.outline_mask_ids = outline_mask_ids;
        self
    }
}

impl<'a, 'ctx> Drop for LineStripBuilder<'a, 'ctx> {
    fn drop(&mut self) {
        if self.outline_mask_ids.is_some() {
            self.builder
                .batches
                .last_mut()
                .unwrap()
                .additional_outline_mask_ids_vertex_ranges
                .push((
                    self.vertex_range.start as u32..self.vertex_range.end as u32,
                    self.outline_mask_ids,
                ));
        }

        self.builder
            .picking_instance_ids_buffer
            .add_n(self.picking_instance_id, self.num_strips_added)
            .ok_or_log_error_once();
        self.builder
            .strips_buffer
            .add_n(self.strip, self.num_strips_added)
            .ok_or_log_error_once();
    }
}
