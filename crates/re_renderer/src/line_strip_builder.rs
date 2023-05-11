use std::ops::Range;

use crate::{
    allocator::CpuWriteGpuReadBuffer,
    renderer::{
        LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags, LineStripInfo, LineVertex,
    },
    Color32, DebugLabel, DepthOffset, OutlineMaskPreference, PickingLayerInstanceId,
    PickingLayerObjectId, RenderContext, Size,
};

/// Builder for a vector of line strips, making it easy to create [`crate::renderer::LineDrawData`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
pub struct LineStripSeriesBuilder {
    pub vertices: Vec<LineVertex>,

    pub batches: Vec<LineBatchInfo>,

    pub strips: Vec<LineStripInfo>,

    /// Buffer for picking instance id - every strip gets its own instance id.
    /// Therefore, there need to be always as many picking instance ids as there are strips.
    pub(crate) picking_instance_ids_buffer: CpuWriteGpuReadBuffer<PickingLayerInstanceId>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl LineStripSeriesBuilder {
    pub fn new(ctx: &RenderContext) -> Self {
        const RESERVE_SIZE: usize = 512;

        // TODO(andreas): Be more resourceful about the size allocated here. Typically we know in advance!
        let picking_instance_ids_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<PickingLayerInstanceId>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                LineDrawData::MAX_NUM_STRIPS,
            );

        Self {
            vertices: Vec::with_capacity(RESERVE_SIZE * 2),
            strips: Vec::with_capacity(RESERVE_SIZE),
            batches: Vec::with_capacity(16),
            picking_instance_ids_buffer,
            radius_boost_in_ui_points_for_outlines: 0.0,
        }
    }

    /// Boosts the size of the points by the given amount of ui-points for the purpose of drawing outlines.
    pub fn radius_boost_in_ui_points_for_outlines(
        mut self,
        radius_boost_in_ui_points_for_outlines: f32,
    ) -> Self {
        self.radius_boost_in_ui_points_for_outlines = radius_boost_in_ui_points_for_outlines;
        self
    }

    /// Start of a new batch.
    pub fn batch(&mut self, label: impl Into<DebugLabel>) -> LineBatchBuilder<'_> {
        self.batches.push(LineBatchInfo {
            label: label.into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            line_vertex_count: 0,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: PickingLayerObjectId::default(),
            depth_offset: 0,
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
    pub fn to_draw_data(
        self,
        ctx: &mut crate::context::RenderContext,
    ) -> Result<LineDrawData, LineDrawDataError> {
        LineDrawData::new(ctx, self)
    }

    pub fn is_empty(&self) -> bool {
        self.strips.is_empty()
    }

    pub fn default_box_flags() -> LineStripFlags {
        LineStripFlags::FLAG_CAP_END_ROUND
            | LineStripFlags::FLAG_CAP_START_ROUND
            | LineStripFlags::FLAG_CAP_END_EXTEND_OUTWARDS
            | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS
    }
}

pub struct LineBatchBuilder<'a>(&'a mut LineStripSeriesBuilder);

impl<'a> Drop for LineBatchBuilder<'a> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().line_vertex_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl<'a> LineBatchBuilder<'a> {
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

    /// Adds a 3D series of line connected points.
    pub fn add_strip(&mut self, points: impl Iterator<Item = glam::Vec3>) -> LineStripBuilder<'_> {
        let old_strip_count = self.0.strips.len();
        let old_vertex_count = self.0.vertices.len();
        let strip_index = old_strip_count as _;

        self.add_vertices(points, strip_index);
        let new_vertex_count = self.0.vertices.len();

        self.0.strips.push(LineStripInfo::default());
        let new_strip_count = self.0.strips.len();

        LineStripBuilder {
            builder: self.0,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_instance_id: PickingLayerInstanceId::default(),
            vertex_range: old_vertex_count..new_vertex_count,
            strip_range: old_strip_count..new_strip_count,
        }
    }

    /// Adds a single 3D line segment connecting two points.
    #[inline]
    pub fn add_segment(&mut self, a: glam::Vec3, b: glam::Vec3) -> LineStripBuilder<'_> {
        self.add_strip([a, b].into_iter())
    }

    /// Adds a series of unconnected 3D line segments.
    pub fn add_segments(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec3, glam::Vec3)>,
    ) -> LineStripBuilder<'_> {
        debug_assert_eq!(
            self.0.strips.len(),
            self.0.picking_instance_ids_buffer.num_written()
        );

        let old_strip_count = self.0.strips.len();
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

        self.0
            .strips
            .extend(std::iter::repeat(LineStripInfo::default()).take(num_strips_added));
        let new_strip_count = self.0.strips.len();

        LineStripBuilder {
            builder: self.0,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_instance_id: PickingLayerInstanceId::default(),
            vertex_range: old_vertex_count..new_vertex_count,
            strip_range: old_strip_count..new_strip_count,
        }
    }

    /// Add box outlines from a unit cube transformed by `transform`.
    ///
    /// Internally adds 12 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_box_outline(&mut self, transform: glam::Affine3A) -> LineStripBuilder<'_> {
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
        .flags(LineStripSeriesBuilder::default_box_flags())
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
    ) -> LineStripBuilder<'_> {
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
        .flags(LineStripSeriesBuilder::default_box_flags())
    }

    /// Adds a 2D series of line connected points.
    ///
    /// Uses autogenerated depth value.
    #[inline]
    pub fn add_strip_2d(
        &mut self,
        points: impl Iterator<Item = glam::Vec2>,
    ) -> LineStripBuilder<'_> {
        self.add_strip(points.map(|p| p.extend(0.0)))
            .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }

    /// Adds a single 2D line segment connecting two points. Uses autogenerated depth value.
    #[inline]
    pub fn add_segment_2d(&mut self, a: glam::Vec2, b: glam::Vec2) -> LineStripBuilder<'_> {
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
    ) -> LineStripBuilder<'_> {
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
    ) -> LineStripBuilder<'_> {
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
    ) -> LineStripBuilder<'_> {
        self.add_rectangle_outline(
            min.extend(0.0),
            glam::Vec3::X * (max.x - min.x),
            glam::Vec3::Y * (max.y - min.y),
        )
        .flags(LineStripFlags::FLAG_FORCE_ORTHO_SPANNING)
    }
}

pub struct LineStripBuilder<'a> {
    builder: &'a mut LineStripSeriesBuilder,
    outline_mask_ids: OutlineMaskPreference,
    picking_instance_id: PickingLayerInstanceId,
    vertex_range: Range<usize>,
    strip_range: Range<usize>,
}

impl<'a> LineStripBuilder<'a> {
    #[inline]
    pub fn radius(self, radius: Size) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.radius = radius;
        }
        self
    }

    #[inline]
    pub fn color(self, color: Color32) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.color = color;
        }
        self
    }

    /// Adds (!) flags to the line strip.
    #[inline]
    pub fn flags(self, flags: LineStripFlags) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.flags |= flags;
        }
        self
    }

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

impl<'a> Drop for LineStripBuilder<'a> {
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
            .extend(std::iter::repeat(self.picking_instance_id).take(self.strip_range.len()));
    }
}
