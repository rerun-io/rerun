use crate::{
    allocator::CpuWriteGpuReadBuffer,
    draw_phases::PickingLayerObjectId,
    renderer::{
        PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
        PointCloudVertex,
    },
    Color32, DebugLabel, DepthOffset, OutlineMaskPreference, PickingLayerInstanceId, RenderContext,
    Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
pub struct PointCloudBuilder {
    // Size of `point`/color` must be equal.
    pub vertices: Vec<PointCloudVertex>,

    pub(crate) color_buffer: CpuWriteGpuReadBuffer<Color32>,
    pub(crate) picking_instance_ids_buffer: CpuWriteGpuReadBuffer<PickingLayerInstanceId>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl PointCloudBuilder {
    pub fn new(ctx: &RenderContext) -> Self {
        const RESERVE_SIZE: usize = 512;

        // TODO(andreas): Be more resourceful about the size allocated here. Typically we know in advance!
        let color_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<Color32>(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            PointCloudDrawData::MAX_NUM_POINTS,
        );
        let picking_instance_ids_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<PickingLayerInstanceId>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                PointCloudDrawData::MAX_NUM_POINTS,
            );

        Self {
            vertices: Vec::with_capacity(RESERVE_SIZE),
            color_buffer,
            picking_instance_ids_buffer,
            batches: Vec::with_capacity(16),
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
    #[inline]
    pub fn batch(&mut self, label: impl Into<DebugLabel>) -> PointCloudBatchBuilder<'_> {
        self.batches.push(PointCloudBatchInfo {
            label: label.into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: PointCloudBatchFlags::FLAG_ENABLE_SHADING,
            point_count: 0,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
        });

        PointCloudBatchBuilder(self)
    }

    // Iterate over all batches, yielding the batch info and a point vertex iterator.
    pub fn iter_vertices_by_batch(
        &self,
    ) -> impl Iterator<
        Item = (
            &PointCloudBatchInfo,
            impl Iterator<Item = &PointCloudVertex>,
        ),
    > {
        let mut vertex_offset = 0;
        self.batches.iter().map(move |batch| {
            let out = (
                batch,
                self.vertices
                    .iter()
                    .skip(vertex_offset)
                    .take(batch.point_count as usize),
            );
            vertex_offset += batch.point_count as usize;
            out
        })
    }

    /// Finalizes the builder and returns a point cloud draw data with all the points added so far.
    pub fn to_draw_data(
        self,
        ctx: &mut crate::context::RenderContext,
    ) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(ctx, self)
    }
}

pub struct PointCloudBatchBuilder<'a>(&'a mut PointCloudBuilder);

impl<'a> Drop for PointCloudBatchBuilder<'a> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().point_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl<'a> PointCloudBatchBuilder<'a> {
    #[inline]
    fn batch_mut(&mut self) -> &mut PointCloudBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on PointCloudBatchBuilder creation")
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

    /// Sets the depth offset for the entire batch.
    #[inline]
    pub fn depth_offset(mut self, depth_offset: DepthOffset) -> Self {
        self.batch_mut().depth_offset = depth_offset;
        self
    }

    /// Add several 3D points
    ///
    /// Returns a `PointBuilder` which can be used to set the colors, radii, and user-data for the points.
    ///
    /// Will *always* add `num_points`, no matter how many elements are in the iterators.
    /// Missing elements will be filled up with defaults (in case of positions that's the origin)
    ///
    /// TODO(#957): Clamps number of points to the allowed per-builder maximum.
    #[inline]
    pub fn add_points(
        mut self,
        mut num_points: usize,
        positions: impl Iterator<Item = glam::Vec3>,
        radii: impl Iterator<Item = Size>,
        colors: impl Iterator<Item = Color32>,
        picking_instance_ids: impl Iterator<Item = PickingLayerInstanceId>,
    ) -> Self {
        // TODO(jleibs): Figure out if we can plumb-through proper support for `Iterator::size_hints()`
        // or potentially make `FixedSizedIterator` work correctly. This should be possible size the
        // underlying arrow structures are of known-size, but carries some complexity with the amount of
        // chaining, joining, filtering, etc. that happens along the way.
        crate::profile_function!();

        debug_assert_eq!(self.0.vertices.len(), self.0.color_buffer.num_written());
        debug_assert_eq!(
            self.0.vertices.len(),
            self.0.picking_instance_ids_buffer.num_written()
        );

        if num_points + self.0.vertices.len() > PointCloudDrawData::MAX_NUM_POINTS {
            re_log::error_once!(
                "Reached maximum number of supported points of {}.
     See also https://github.com/rerun-io/rerun/issues/957",
                PointCloudDrawData::MAX_NUM_POINTS
            );
            num_points = PointCloudDrawData::MAX_NUM_POINTS - self.0.vertices.len();
        }
        if num_points == 0 {
            return self;
        }
        self.batch_mut().point_count += num_points as u32;

        {
            crate::profile_scope!("positions");
            let num_before = self.0.vertices.len();
            self.0.vertices.extend(
                positions
                    .take(num_points)
                    .zip(radii.take(num_points))
                    .map(|(position, radius)| PointCloudVertex { position, radius }),
            );
            // Fill up with defaults. Doing this in a separate step is faster than chaining the iterator.
            let num_default = num_points - (self.0.vertices.len() - num_before);
            self.0.vertices.extend(
                std::iter::repeat(PointCloudVertex {
                    position: glam::Vec3::ZERO,
                    radius: Size::AUTO,
                })
                .take(num_default),
            );
        }
        {
            crate::profile_scope!("colors");
            let num_written = self.0.color_buffer.extend(colors.take(num_points));
            // Fill up with defaults. Doing this in a separate step is faster than chaining the iterator.
            self.0
                .color_buffer
                .extend(std::iter::repeat(Color32::TRANSPARENT).take(num_points - num_written));
        }
        {
            crate::profile_scope!("picking_instance_ids");
            let num_written = self
                .0
                .picking_instance_ids_buffer
                .extend(picking_instance_ids.take(num_points));
            // Fill up with defaults. Doing this in a separate step is faster than chaining the iterator.
            self.0.picking_instance_ids_buffer.extend(
                std::iter::repeat(PickingLayerInstanceId::default()).take(num_points - num_written),
            );
        }

        self
    }

    /// Adds several 2D points. Uses an autogenerated depth value, the same for all points passed.
    ///
    /// Will *always* add `num_points`, no matter how many elements are in the iterators.
    /// Missing elements will be filled up with defaults (in case of positions that's the origin)
    #[inline]
    pub fn add_points_2d(
        self,
        num_points: usize,
        positions: impl Iterator<Item = glam::Vec2>,
        radii: impl Iterator<Item = Size>,
        colors: impl Iterator<Item = Color32>,
        picking_instance_ids: impl Iterator<Item = PickingLayerInstanceId>,
    ) -> Self {
        self.add_points(
            num_points,
            positions.map(|p| p.extend(0.0)),
            radii,
            colors,
            picking_instance_ids,
        )
        .flags(PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES)
    }

    /// Adds (!) flags for this batch.
    pub fn flags(mut self, flags: PointCloudBatchFlags) -> Self {
        self.batch_mut().flags |= flags;
        self
    }

    /// Sets the picking object id for the current batch.
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    /// Pushes additional outline mask ids for a specific range of points.
    /// The range is relative to this batch.
    ///
    /// Prefer the `overall_outline_mask_ids` setting to set the outline mask ids for the entire batch whenever possible!
    #[inline]
    pub fn push_additional_outline_mask_ids_for_range(
        mut self,
        range: std::ops::Range<u32>,
        ids: OutlineMaskPreference,
    ) -> Self {
        self.batch_mut()
            .additional_outline_mask_ids_vertex_ranges
            .push((range, ids));
        self
    }
}
