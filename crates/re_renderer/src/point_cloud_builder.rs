use itertools::izip;

use re_log::ResultExt;

use crate::{
    allocator::CpuWriteGpuReadBuffer,
    draw_phases::PickingLayerObjectId,
    renderer::{
        data_texture_source_buffer_element_count, PointCloudBatchFlags, PointCloudBatchInfo,
        PointCloudDrawData, PointCloudDrawDataError, PositionRadius,
    },
    Color32, DebugLabel, DepthOffset, OutlineMaskPreference, PickingLayerInstanceId, RenderContext,
    Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
pub struct PointCloudBuilder {
    // Size of `point`/color` must be equal.
    pub vertices: Vec<PositionRadius>,

    pub(crate) color_buffer: CpuWriteGpuReadBuffer<Color32>,
    pub(crate) scale_buffer: CpuWriteGpuReadBuffer<glam::Vec4>, // TODO: optional
    pub(crate) rotation_buffer: CpuWriteGpuReadBuffer<glam::Quat>, // TODO: optional
    pub(crate) picking_instance_ids_buffer: CpuWriteGpuReadBuffer<PickingLayerInstanceId>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,

    max_num_points: usize,
}

impl PointCloudBuilder {
    pub fn new(ctx: &RenderContext, max_num_points: u32) -> Self {
        let max_texture_dimension_2d = ctx.device.limits().max_texture_dimension_2d;

        let color_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<Color32>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                data_texture_source_buffer_element_count(
                    PointCloudDrawData::COLOR_TEXTURE_FORMAT,
                    max_num_points,
                    max_texture_dimension_2d,
                ),
            )
            .expect("Failed to allocate color buffer"); // TODO(#3408): Should never happen but should propagate error anyways

        let scale_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<glam::Vec4>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                data_texture_source_buffer_element_count(
                    PointCloudDrawData::SCALE_TEXTURE_FORMAT,
                    max_num_points,
                    max_texture_dimension_2d,
                ),
            )
            .expect("Failed to allocate scale buffer"); // TODO(#3408): Should never happen but should propagate error anyways

        let rotation_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<glam::Quat>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                data_texture_source_buffer_element_count(
                    PointCloudDrawData::ROTATION_TEXTURE_FORMAT,
                    max_num_points,
                    max_texture_dimension_2d,
                ),
            )
            .expect("Failed to allocate rotation buffer"); // TODO(#3408): Should never happen but should propagate error anyways

        let picking_instance_ids_buffer = ctx
            .cpu_write_gpu_read_belt
            .lock()
            .allocate::<PickingLayerInstanceId>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                data_texture_source_buffer_element_count(
                    PointCloudDrawData::PICKING_INSTANCE_ID_TEXTURE_FORMAT,
                    max_num_points,
                    max_texture_dimension_2d,
                ),
            )
            .expect("Failed to allocate picking layer buffer"); // TODO(#3408): Should never happen but should propagate error anyways

        Self {
            vertices: Vec::with_capacity(max_num_points as usize),
            color_buffer,
            scale_buffer,
            rotation_buffer,
            picking_instance_ids_buffer,
            batches: Vec::with_capacity(16),
            radius_boost_in_ui_points_for_outlines: 0.0,
            max_num_points: max_num_points as usize,
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
    ) -> impl Iterator<Item = (&PointCloudBatchInfo, impl Iterator<Item = &PositionRadius>)> {
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
    pub fn into_draw_data(
        self,
        ctx: &crate::context::RenderContext,
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
    /// Will add all positions.
    /// Missing radii will default to `Size::AUTO`.
    /// Missing colors will default to white.
    #[inline]
    pub fn add_points(
        mut self,
        positions: &[glam::Vec3],
        radii: &[Size],
        colors: &[Color32],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        // TODO(jleibs): Figure out if we can plumb-through proper support for `Iterator::size_hints()`
        // or potentially make `FixedSizedIterator` work correctly. This should be possible size the
        // underlying arrow structures are of known-size, but carries some complexity with the amount of
        // chaining, joining, filtering, etc. that happens along the way.
        re_tracing::profile_function!();

        let mut num_points = positions.len();

        debug_assert_eq!(self.0.vertices.len(), self.0.color_buffer.num_written());
        debug_assert_eq!(
            self.0.vertices.len(),
            self.0.picking_instance_ids_buffer.num_written()
        );

        if num_points + self.0.vertices.len() > self.0.max_num_points {
            re_log::error_once!(
                "Reserved space for {} points, but reached {}. Clamping to previously set maximum",
                self.0.max_num_points,
                num_points + self.0.vertices.len()
            );
            num_points = self.0.max_num_points - self.0.vertices.len();
        }
        if num_points == 0 {
            return self;
        }

        // Shorten slices if needed:
        let positions = &positions[0..num_points.min(positions.len())];
        let radii = &radii[0..num_points.min(radii.len())];
        let colors = &colors[0..num_points.min(colors.len())];
        let picking_ids = &picking_ids[0..num_points.min(picking_ids.len())];

        self.batch_mut().point_count += num_points as u32;

        {
            re_tracing::profile_scope!("positions & radii");
            self.0.vertices.extend(
                izip!(
                    positions.iter().copied(),
                    radii.iter().copied().chain(std::iter::repeat(Size::AUTO))
                )
                .map(|(pos, radius)| PositionRadius { pos, radius }),
            );
        }
        {
            re_tracing::profile_scope!("colors");

            self.0
                .color_buffer
                .extend_from_slice(colors)
                .ok_or_log_error();

            // Fill up with defaults. Doing this in a separate step is faster than chaining the iterator.
            self.0
                .color_buffer
                .fill_n(Color32::WHITE, num_points.saturating_sub(colors.len()))
                .ok_or_log_error();
        }
        {
            re_tracing::profile_scope!("picking_ids");

            self.0
                .picking_instance_ids_buffer
                .extend_from_slice(picking_ids)
                .ok_or_log_error();

            // Fill up with defaults. Doing this in a separate step is faster than chaining the iterator.
            self.0
                .picking_instance_ids_buffer
                .fill_n(
                    PickingLayerInstanceId::default(),
                    num_points.saturating_sub(picking_ids.len()),
                )
                .ok_or_log_error();
        }

        self
    }

    /// Adds several 2D points (assumes Z=0). Uses an autogenerated depth value, the same for all points passed.
    ///
    /// Will add all positions.
    /// Missing radii will default to `Size::AUTO`.
    /// Missing colors will default to white.
    #[inline]
    pub fn add_points_2d(
        self,
        positions: &[glam::Vec3],
        radii: &[Size],
        colors: &[Color32],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        re_tracing::profile_function!();
        self.add_points(positions, radii, colors, picking_ids)
            .flags(PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES)
    }

    /// Adds (!) flags for this batch.
    #[inline]
    pub fn flags(mut self, flags: PointCloudBatchFlags) -> Self {
        self.batch_mut().flags |= flags;
        self
    }

    /// Sets the picking object id for the current batch.
    #[inline]
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    pub fn push_scales3(&mut self, scales: &[glam::Vec3]) {
        // TODO: handle only some point clouds having scales
        re_tracing::profile_function!();
        let scales4 = scales
            .iter()
            .copied()
            .map(|s| glam::Vec4::new(s.x, s.y, s.z, 1.0));
        self.0
            .scale_buffer
            .extend(scales4.into_iter())
            .unwrap_debug_or_log_error();
    }

    pub fn push_rotations(&mut self, rotations: &[glam::Quat]) {
        // TODO: handle only some point clouds having rotations
        re_tracing::profile_function!();
        self.0
            .rotation_buffer
            .extend_from_slice(rotations)
            .unwrap_debug_or_log_error();
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
