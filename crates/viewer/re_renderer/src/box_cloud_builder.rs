use itertools::izip;

use re_log::ResultExt as _;

use crate::{
    Color32, CpuWriteGpuReadError, DebugLabel, DepthOffset, OutlineMaskPreference,
    PickingLayerInstanceId, RenderContext,
    allocator::DataTextureSource,
    draw_phases::PickingLayerObjectId,
    renderer::{
        BoxCloudBatchFlags, BoxCloudBatchInfo, BoxCloudDrawData, BoxCloudDrawDataError,
    },
};

/// Builder for box clouds, making it easy to create [`crate::renderer::BoxCloudDrawData`].
///
/// This is a high-performance renderer for large numbers of axis-aligned boxes,
/// inspired by the point cloud renderer. It stores box data in GPU textures and
/// generates box geometry procedurally in the vertex shader.
pub struct BoxCloudBuilder<'ctx> {
    pub(crate) ctx: &'ctx RenderContext,

    // Each box is stored as 2 Vec4s (2 texels):
    // - Vec4 0: (center.x, center.y, center.z, half_size.x)
    // - Vec4 1: (half_size.y, half_size.z, 0, 0)
    pub(crate) position_halfsize_buffer: DataTextureSource<'ctx, glam::Vec4>,

    pub(crate) color_buffer: DataTextureSource<'ctx, Color32>,
    pub(crate) picking_instance_ids_buffer: DataTextureSource<'ctx, PickingLayerInstanceId>,

    pub(crate) batches: Vec<BoxCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl<'ctx> BoxCloudBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            position_halfsize_buffer: DataTextureSource::new(ctx),
            color_buffer: DataTextureSource::new(ctx),
            picking_instance_ids_buffer: DataTextureSource::new(ctx),
            batches: Vec::with_capacity(16),
            radius_boost_in_ui_points_for_outlines: 0.0,
        }
    }

    /// Returns number of boxes that can be added without reallocation.
    pub fn reserve(
        &mut self,
        expected_number_of_additional_boxes: usize,
    ) -> Result<usize, CpuWriteGpuReadError> {
        // Each box uses 2 Vec4s in position_halfsize_buffer
        let position_halfsize_capacity = self
            .position_halfsize_buffer
            .reserve(expected_number_of_additional_boxes * 2)?;
        let color_capacity = self
            .color_buffer
            .reserve(expected_number_of_additional_boxes)?;
        let picking_capacity = self
            .picking_instance_ids_buffer
            .reserve(expected_number_of_additional_boxes)?;

        // Return the minimum capacity across all buffers
        // position_halfsize_capacity is in Vec4s, so divide by 2 for box count
        Ok((position_halfsize_capacity / 2)
            .min(color_capacity)
            .min(picking_capacity))
    }

    /// Boosts the size of the box edges by the given amount of ui-points for the purpose of drawing outlines.
    pub fn radius_boost_in_ui_points_for_outlines(
        &mut self,
        radius_boost_in_ui_points_for_outlines: f32,
    ) {
        self.radius_boost_in_ui_points_for_outlines = radius_boost_in_ui_points_for_outlines;
    }

    /// Start of a new batch.
    #[inline]
    pub fn batch(&mut self, label: impl Into<DebugLabel>) -> BoxCloudBatchBuilder<'_, 'ctx> {
        self.batches.push(BoxCloudBatchInfo {
            label: label.into(),
            ..BoxCloudBatchInfo::default()
        });

        BoxCloudBatchBuilder(self)
    }

    #[inline]
    pub fn batch_with_info(
        &mut self,
        info: BoxCloudBatchInfo,
    ) -> BoxCloudBatchBuilder<'_, 'ctx> {
        self.batches.push(info);

        BoxCloudBatchBuilder(self)
    }

    /// Finalizes the builder and returns a box cloud draw data with all the boxes added so far.
    pub fn into_draw_data(self) -> Result<BoxCloudDrawData, BoxCloudDrawDataError> {
        BoxCloudDrawData::new(self)
    }
}

pub struct BoxCloudBatchBuilder<'a, 'ctx>(&'a mut BoxCloudBuilder<'ctx>);

impl Drop for BoxCloudBatchBuilder<'_, '_> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().box_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl BoxCloudBatchBuilder<'_, '_> {
    #[inline]
    fn batch_mut(&mut self) -> &mut BoxCloudBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on BoxCloudBatchBuilder creation")
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

    /// Add several 3D boxes
    ///
    /// Returns a `BoxBuilder` which can be used to set the colors and user-data for the boxes.
    ///
    /// Will add all boxes.
    /// Missing colors will default to white.
    #[inline]
    pub fn add_boxes(
        mut self,
        centers: &[glam::Vec3],
        half_sizes: &[glam::Vec3],
        colors: &[Color32],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        re_tracing::profile_function!();

        debug_assert_eq!(
            self.0.position_halfsize_buffer.len() / 2,
            self.0.color_buffer.len()
        );
        debug_assert_eq!(
            self.0.position_halfsize_buffer.len() / 2,
            self.0.picking_instance_ids_buffer.len()
        );

        // Do a reserve ahead of time, to check whether we're hitting the data texture limit.
        // Each box uses 2 Vec4s in position_halfsize_buffer.
        let Some(num_available_vec4s) = self
            .0
            .position_halfsize_buffer
            .reserve(centers.len() * 2)
            .ok_or_log_error()
        else {
            return self;
        };

        let num_available_boxes = num_available_vec4s / 2;
        let num_boxes = if centers.len() > num_available_boxes {
            re_log::error_once!(
                "Reached maximum number of boxes for box cloud of {}. Ignoring all excess boxes.",
                self.0.position_halfsize_buffer.len() / 2 + num_available_boxes
            );
            num_available_boxes
        } else {
            centers.len()
        };

        if num_boxes == 0 {
            return self;
        }

        // Shorten slices if needed:
        let centers = &centers[0..num_boxes.min(centers.len())];
        let half_sizes = &half_sizes[0..num_boxes.min(half_sizes.len())];
        let colors = &colors[0..num_boxes.min(colors.len())];
        let picking_ids = &picking_ids[0..num_boxes.min(picking_ids.len())];

        self.batch_mut().box_count += num_boxes as u32;

        {
            re_tracing::profile_scope!("positions & half_sizes");

            // Pack each box as 2 Vec4s
            let mut box_data = Vec::with_capacity(num_boxes * 2);

            if centers.len() == half_sizes.len() {
                // Optimize common-case with simpler iterators.
                for (center, half_size) in izip!(centers.iter().copied(), half_sizes.iter().copied()) {
                    box_data.push(glam::Vec4::new(center.x, center.y, center.z, half_size.x));
                    box_data.push(glam::Vec4::new(half_size.y, half_size.z, 0.0, 0.0));
                }
            } else {
                for (center, half_size) in izip!(
                    centers.iter().copied(),
                    half_sizes
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(*half_sizes.last().unwrap_or(&glam::Vec3::ONE)))
                ) {
                    box_data.push(glam::Vec4::new(center.x, center.y, center.z, half_size.x));
                    box_data.push(glam::Vec4::new(half_size.y, half_size.z, 0.0, 0.0));
                }
            }

            self.0
                .position_halfsize_buffer
                .extend_from_slice(&box_data)
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
                .add_n(Color32::WHITE, num_boxes.saturating_sub(colors.len()))
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
                    num_boxes.saturating_sub(picking_ids.len()),
                )
                .ok_or_log_error();
        }

        self
    }

    /// Adds (!) flags for this batch.
    #[inline]
    pub fn flags(mut self, flags: BoxCloudBatchFlags) -> Self {
        self.batch_mut().flags |= flags;
        self
    }

    /// Sets the picking object id for the current batch.
    #[inline]
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    /// Pushes additional outline mask ids for a specific range of boxes.
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
