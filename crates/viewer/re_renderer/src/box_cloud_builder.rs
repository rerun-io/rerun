use re_log::ResultExt as _;

use crate::{
    Color32, DebugLabel, DepthOffset, OutlineMaskPreference, PickingLayerInstanceId,
    RenderContext,
    draw_phases::PickingLayerObjectId,
    renderer::{BoxCloudBatchFlags, BoxCloudBatchInfo, BoxCloudDrawData, BoxCloudDrawDataError},
};

/// Instance data for a single box in the box cloud.
#[derive(Clone, Copy)]
pub struct BoxInstance {
    pub center: glam::Vec3,
    pub half_size: glam::Vec3,
    pub color: Color32,
    pub picking_instance_id: PickingLayerInstanceId,
}

/// Builder for box clouds, making it easy to create [`crate::renderer::BoxCloudDrawData`].
///
/// This is a high-performance renderer for large numbers of axis-aligned boxes,
/// using GPU instancing similar to the mesh renderer. It stores box data in an
/// instance buffer and uses a shared vertex buffer for unit cube geometry.
pub struct BoxCloudBuilder<'ctx> {
    pub(crate) ctx: &'ctx RenderContext,
    pub(crate) instances: Vec<BoxInstance>,
    pub(crate) batches: Vec<BoxCloudBatchInfo>,
}

impl<'ctx> BoxCloudBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            instances: Vec::new(),
            batches: Vec::with_capacity(16),
        }
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

        let num_boxes = centers.len();
        if num_boxes == 0 {
            return self;
        }

        self.batch_mut().box_count += num_boxes as u32;

        // Extend instances
        for i in 0..num_boxes {
            let center = centers.get(i).copied().unwrap_or(glam::Vec3::ZERO);
            let half_size = half_sizes
                .get(i)
                .copied()
                .or_else(|| half_sizes.last().copied())
                .unwrap_or(glam::Vec3::ONE);
            let color = colors
                .get(i)
                .copied()
                .or_else(|| colors.last().copied())
                .unwrap_or(Color32::WHITE);
            let picking_id = picking_ids
                .get(i)
                .copied()
                .or_else(|| picking_ids.last().copied())
                .unwrap_or_default();

            self.0.instances.push(BoxInstance {
                center,
                half_size,
                color,
                picking_instance_id: picking_id,
            });
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
