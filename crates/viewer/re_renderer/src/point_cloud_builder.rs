use re_log::{ResultExt as _, debug_assert_eq};

use crate::allocator::DataTextureSource;
use crate::draw_phases::PickingLayerObjectId;
use crate::renderer::gpu_data::PositionRadius;
use crate::renderer::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
};
use crate::{
    Color32, CpuWriteGpuReadError, DepthOffset, Label, OutlineMaskPreference,
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
        re_tracing::profile_function_if!(100_000 < expected_number_of_additional_points);

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
    pub fn batch(&mut self, label: impl Into<Label>) -> PointCloudBatchBuilder<'_, 'ctx> {
        self.batches.push(PointCloudBatchInfo {
            label: label.into(),
            ..PointCloudBatchInfo::default()
        });

        PointCloudBatchBuilder {
            builder: self,
            object_space_bounding_box_is_complete: false,
        }
    }

    #[inline]
    pub fn batch_with_info(
        &mut self,
        info: PointCloudBatchInfo,
    ) -> PointCloudBatchBuilder<'_, 'ctx> {
        let bounds_are_complete = !info.object_space_bounding_box.is_nothing();
        self.batches.push(info);

        PointCloudBatchBuilder {
            builder: self,
            object_space_bounding_box_is_complete: bounds_are_complete,
        }
    }

    /// Finalizes the builder and returns a point cloud draw data with all the points added so far.
    pub fn into_draw_data(self) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(self)
    }
}

pub struct PointCloudBatchBuilder<'a, 'ctx> {
    builder: &'a mut PointCloudBuilder<'ctx>,
    object_space_bounding_box_is_complete: bool,
}

impl Drop for PointCloudBatchBuilder<'_, '_> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.builder.batches.last().unwrap().point_count == 0 {
            self.builder.batches.pop();
        }
    }
}

impl PointCloudBatchBuilder<'_, '_> {
    #[inline]
    fn batch_mut(&mut self) -> &mut PointCloudBatchInfo {
        self.builder
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

    /// Provides the complete object-space bounds for the batch.
    ///
    /// This avoids recomputing them while adding points.
    #[inline]
    pub fn object_space_bounding_box(
        mut self,
        object_space_bounding_box: macaw::BoundingBox,
    ) -> Self {
        self.batch_mut().object_space_bounding_box = object_space_bounding_box;
        self.object_space_bounding_box_is_complete = true;
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

    /// Add several 3D points.
    ///
    /// If possible, prefer to using [`Self::add_points`] instead,
    /// which avoids doing any extra allocations.
    ///
    /// Returns a `PointBuilder` which can be used to set the colors, radii, and user-data for the points.
    ///
    /// Will add all positions.
    /// Missing radii will default to `Size::AUTO`.
    /// Missing colors will default to white.
    #[inline]
    pub fn add_points_slow(
        self,
        positions: &[glam::Vec3],
        radii: &[Size],
        colors: &[Color32],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        re_tracing::profile_function!();

        let positions_and_radii = PositionRadius::from_many(positions, radii);
        self.add_points(&positions_and_radii, colors, picking_ids)
    }

    /// Add several 3D points
    ///
    /// Returns a `PointBuilder` which can be used to set the colors, radii, and user-data for the points.
    ///
    /// Will add all positions.
    /// Missing colors will default to white.
    #[inline]
    pub fn add_points(
        mut self,
        positions_and_radii: &[PositionRadius],
        colors: &[Color32],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        re_tracing::profile_function!();

        debug_assert_eq!(
            self.builder.position_radius_buffer.len(),
            self.builder.color_buffer.len()
        );
        debug_assert_eq!(
            self.builder.position_radius_buffer.len(),
            self.builder.picking_instance_ids_buffer.len()
        );

        let num_points = positions_and_radii.len();

        // Do a reserve ahead of time, to check whether we're hitting the data texture limit.
        // The limit is the same for all data textures, so we only need to check one.
        let Some(num_available_points) = self
            .builder
            .position_radius_buffer
            .reserve(num_points)
            .ok_or_log_error()
        else {
            return self;
        };

        let num_points = if num_points > num_available_points {
            re_log::error_once!(
                "Reached maximum number of points for point cloud of {}. Ignoring all excess points.",
                self.builder.position_radius_buffer.len() + num_available_points
            );
            num_available_points
        } else {
            num_points
        };

        if num_points == 0 {
            return self;
        }

        // Shorten slices if needed:
        let positions_and_radii =
            &positions_and_radii[0..num_points.min(positions_and_radii.len())];
        let colors = &colors[0..num_points.min(colors.len())];
        let picking_ids = &picking_ids[0..num_points.min(picking_ids.len())];

        let bounds_are_complete = self.object_space_bounding_box_is_complete;
        let batch = self.batch_mut();
        batch.point_count += num_points as u32;

        if !bounds_are_complete {
            batch.object_space_bounding_box =
                batch
                    .object_space_bounding_box
                    .union(crate::util::bounding_box_from_points(
                        positions_and_radii.iter().map(|point| point.pos),
                    ));
        }

        // Retain object-space positions for transparent batches that opted into back-to-front
        // sorting by supplying a cache (see [`Self::sort_order`]). Coplanar clouds (e.g. 2D points)
        // skip this and just alpha-blend in insertion order.
        // Only the flag/cache set *before* adding points is honored.
        let wants_sorting = batch
            .flags
            .contains(PointCloudBatchFlags::FLAG_PREMULTIPLIED_ALPHA)
            && batch.sort_order_cache.is_some();
        if wants_sorting {
            re_tracing::profile_scope!("sort_positions");
            let sort_positions = self.batch_mut().sort_positions.get_or_insert_with(Vec::new);
            sort_positions.extend(positions_and_radii.iter().map(|pr| pr.pos));
        }

        {
            re_tracing::profile_scope!("positions_and_radii");
            self.builder
                .position_radius_buffer
                .extend_from_slice(positions_and_radii)
                .ok_or_log_error();
        }
        {
            re_tracing::profile_scope!("colors");

            self.builder
                .color_buffer
                .extend_from_slice(colors)
                .ok_or_log_error();
            self.builder
                .color_buffer
                .add_n(Color32::WHITE, num_points.saturating_sub(colors.len())) // TODO(emilk): don't use a hard-coded default color here
                .ok_or_log_error();
        }
        {
            re_tracing::profile_scope!("picking_ids");

            self.builder
                .picking_instance_ids_buffer
                .extend_from_slice(picking_ids)
                .ok_or_log_error();
            self.builder
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
        self.add_points_slow(positions, radii, colors, picking_ids)
            .flags(PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES)
    }

    /// Adds (!) flags for this batch.
    #[inline]
    pub fn flags(mut self, flags: PointCloudBatchFlags) -> Self {
        self.batch_mut().flags |= flags;
        self
    }

    /// Sets [`PointCloudBatchFlags::FLAG_ENABLE_SHADING`] for this batch.
    #[inline]
    pub fn enable_shading(mut self, enabled: bool) -> Self {
        self.batch_mut()
            .flags
            .set(PointCloudBatchFlags::FLAG_ENABLE_SHADING, enabled);
        self
    }

    /// Renders this batch with premultiplied-alpha blending in the transparent draw phase.
    ///
    /// Enable this when the batch contains any semi-transparent points.
    /// Opaque batches should leave this off so they can use the faster alpha-to-coverage path
    /// and write depth.
    ///
    /// Alpha blending does not enable per-point sorting.
    /// Call [`Self::sort_order`] before adding points to opt in; batches such as coplanar 2D points
    /// can instead preserve insertion order.
    /// See <https://github.com/rerun-io/rerun/issues/1611> for remaining ordering limitations.
    #[inline]
    pub fn enable_alpha_blending(mut self, enabled: bool) -> Self {
        self.batch_mut()
            .flags
            .set(PointCloudBatchFlags::FLAG_PREMULTIPLIED_ALPHA, enabled);
        self
    }

    /// Sets the picking object id for the current batch.
    #[inline]
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    /// Opts into per-frame back-to-front sorting with a caller-owned cache.
    ///
    /// Sorting is independent of alpha blending because some alpha-blended batches already have a
    /// suitable insertion order.
    ///
    /// The cache must be unique among concurrently-drawn clouds.
    /// It keeps independent ordering per view when draw data is shared across views.
    /// The caller owns its lifetime and invalidation, and must enable alpha blending and provide
    /// the cache before adding points.
    #[inline]
    pub fn sort_order(
        mut self,
        sort_order_cache: crate::renderer::PointCloudSortOrderCache,
    ) -> Self {
        self.batch_mut().sort_order_cache = Some(sort_order_cache);
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
