use itertools::{Itertools as _, izip};
use re_log::{ResultExt as _, debug_assert_eq};

use crate::allocator::DataTextureSource;
use crate::draw_phases::PickingLayerObjectId;
use crate::renderer::gpu_data::PositionRadius;
use crate::renderer::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
};
use crate::{
    Color32, CpuWriteGpuReadError, DebugLabel, DepthOffset, OutlineMaskPreference,
    PickingLayerInstanceId, RenderContext, Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
pub struct PointCloudBuilder<'ctx> {
    pub(crate) ctx: &'ctx RenderContext,

    // Size of `point`/color` must be equal.
    pub(crate) position_radius_buffer: DataTextureSource<'ctx, PositionRadius>,

    pub(crate) color_buffer: DataTextureSource<'ctx, Color32>,
    pub(crate) picking_instance_ids_buffer: DataTextureSource<'ctx, PickingLayerInstanceId>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl<'ctx> PointCloudBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            position_radius_buffer: DataTextureSource::new(ctx),
            color_buffer: DataTextureSource::new(ctx),
            picking_instance_ids_buffer: DataTextureSource::new(ctx),
            batches: Vec::with_capacity(16),
            radius_boost_in_ui_points_for_outlines: 0.0,
        }
    }

    /// Returns number of points that can be added without reallocation.
    /// This may be smaller than the requested number if the maximum number of strips is reached.
    pub fn reserve(
        &mut self,
        expected_number_of_additional_points: usize,
    ) -> Result<usize, CpuWriteGpuReadError> {
        // We know that the maximum number is independent of datatype, so we can use the same value for all.
        self.position_radius_buffer
            .reserve(expected_number_of_additional_points)?;
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
            ..PointCloudBatchInfo::default()
        });

        PointCloudBatchBuilder(self)
    }

    #[inline]
    pub fn batch_with_info(
        &mut self,
        info: PointCloudBatchInfo,
    ) -> PointCloudBatchBuilder<'_, 'ctx> {
        self.batches.push(info);

        PointCloudBatchBuilder(self)
    }

    /// Finalizes the builder and returns a point cloud draw data with all the points added so far.
    pub fn into_draw_data(self) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(self)
    }
}

pub struct PointCloudBatchBuilder<'a, 'ctx>(&'a mut PointCloudBuilder<'ctx>);

impl Drop for PointCloudBatchBuilder<'_, '_> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().point_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl PointCloudBatchBuilder<'_, '_> {
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
        re_tracing::profile_function!();

        debug_assert_eq!(
            self.0.position_radius_buffer.len(),
            self.0.color_buffer.len()
        );
        debug_assert_eq!(
            self.0.position_radius_buffer.len(),
            self.0.picking_instance_ids_buffer.len()
        );

        // Do a reserve ahead of time, to check whether we're hitting the data texture limit.
        // The limit is the same for all data textures, so we only need to check one.
        let Some(num_available_points) = self
            .0
            .position_radius_buffer
            .reserve(positions.len())
            .ok_or_log_error()
        else {
            return self;
        };

        let num_points = if positions.len() > num_available_points {
            re_log::error_once!(
                "Reached maximum number of points for point cloud of {}. Ignoring all excess points.",
                self.0.position_radius_buffer.len() + num_available_points
            );
            num_available_points
        } else {
            positions.len()
        };

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

            // TODO(andreas): It would be nice to pass on the iterator as is so we don't have to do yet another
            // copy of the data and instead write into the buffers directly - if done right this should be the fastest.
            // But it's surprisingly tricky to do this effectively.
            let vertices = if positions.len() == radii.len() {
                // Optimize common-case with simpler iterators.
                re_tracing::profile_scope!("collect_vec");
                izip!(positions.iter().copied(), radii.iter().copied())
                    .map(|(pos, radius)| PositionRadius { pos, radius })
                    .collect_vec()
            } else {
                re_tracing::profile_scope!("collect_vec");
                izip!(
                    positions.iter().copied(),
                    radii.iter().copied().chain(std::iter::repeat(
                        *radii.last().unwrap_or(&Size::ONE_UI_POINT)
                    ))
                )
                .map(|(pos, radius)| PositionRadius { pos, radius })
                .collect_vec()
            };

            self.0
                .position_radius_buffer
                .extend_from_slice(&vertices)
                .ok_or_log_error();
        }
        {
            re_tracing::profile_scope!("colors");

            self.0
                .color_buffer
                .extend_from_slice(colors)
                .ok_or_log_error();
            self.0
                .color_buffer
                .add_n(Color32::WHITE, num_points.saturating_sub(colors.len())) // TODO(emilk): don't use a hard-coded default color here
                .ok_or_log_error();
        }
        {
            re_tracing::profile_scope!("picking_ids");

            self.0
                .picking_instance_ids_buffer
                .extend_from_slice(picking_ids)
                .ok_or_log_error();
            self.0
                .picking_instance_ids_buffer
                .add_n(
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
