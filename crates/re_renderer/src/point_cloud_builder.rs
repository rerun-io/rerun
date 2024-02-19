use itertools::izip;

use re_log::ResultExt;

use crate::{
    allocator::DataTextureSource,
    draw_phases::PickingLayerObjectId,
    renderer::{
        PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
        PositionRadius,
    },
    Color32, CpuWriteGpuReadError, DebugLabel, DepthOffset, OutlineMaskPreference,
    PickingLayerInstanceId, RenderContext, Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
pub struct PointCloudBuilder<'ctx> {
    pub(crate) ctx: &'ctx RenderContext,

    // Size of `point`/color` must be equal.
    pub(crate) vertices: Vec<PositionRadius>,

    pub(crate) color_buffer: DataTextureSource<'ctx, Color32>,
    pub(crate) picking_instance_ids_buffer: DataTextureSource<'ctx, PickingLayerInstanceId>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl<'ctx> PointCloudBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            vertices: Vec::new(),
            color_buffer: DataTextureSource::new(ctx),
            picking_instance_ids_buffer: DataTextureSource::new(ctx),
            batches: Vec::with_capacity(16),
            radius_boost_in_ui_points_for_outlines: 0.0,
        }
    }

    pub fn reserve(
        &mut self,
        expected_number_of_additional_points: usize,
    ) -> Result<(), CpuWriteGpuReadError> {
        self.vertices.reserve(expected_number_of_additional_points);
        self.color_buffer
            .reserve(expected_number_of_additional_points)?;
        self.picking_instance_ids_buffer
            .reserve(expected_number_of_additional_points)
    }

    /// Boosts the size of the points by the given amount of ui-points for the purpose of drawing outlines.
    pub fn radius_boost_in_ui_points_for_outlines(
        &mut self,
        radius_boost_in_ui_points_for_outlines: f32,
    ) {
        self.radius_boost_in_ui_points_for_outlines = radius_boost_in_ui_points_for_outlines;
    }

    /// Start of a new batch.
    #[inline]
    pub fn batch(&mut self, label: impl Into<DebugLabel>) -> PointCloudBatchBuilder<'_, 'ctx> {
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
    pub fn into_draw_data(self) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(self)
    }
}

pub struct PointCloudBatchBuilder<'a, 'ctx>(&'a mut PointCloudBuilder<'ctx>);

impl<'a, 'ctx> Drop for PointCloudBatchBuilder<'a, 'ctx> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().point_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl<'a, 'ctx> PointCloudBatchBuilder<'a, 'ctx> {
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

        debug_assert_eq!(self.0.vertices.len(), self.0.color_buffer.len());
        debug_assert_eq!(
            self.0.vertices.len(),
            self.0.picking_instance_ids_buffer.len()
        );

        if positions.is_empty() {
            return self;
        }

        // Shorten slices if needed:
        let radii = &radii[0..positions.len().min(radii.len())];
        let colors = &colors[0..positions.len().min(colors.len())];
        let picking_ids = &picking_ids[0..positions.len().min(picking_ids.len())];

        self.batch_mut().point_count += positions.len() as u32;

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
                .add_n(Color32::WHITE, positions.len().saturating_sub(colors.len()))
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
                .add_n(
                    PickingLayerInstanceId::default(),
                    positions.len().saturating_sub(picking_ids.len()),
                )
                .ok_or_log_error(); // TODO: forward errors here and elsewhere?
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
