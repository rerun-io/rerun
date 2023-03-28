use crate::{
    allocator::CpuWriteGpuReadBuffer,
    renderer::{
        OutlineMaskPreference, PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData,
        PointCloudDrawDataError, PointCloudVertex,
    },
    Color32, DebugLabel, RenderContext, Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
pub struct PointCloudBuilder<PerPointUserData> {
    // Size of `point`/color`/`per_point_user_data` must be equal.
    pub vertices: Vec<PointCloudVertex>,

    pub(crate) color_buffer: CpuWriteGpuReadBuffer<Color32>,
    pub user_data: Vec<PerPointUserData>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) radius_boost_in_ui_points_for_outlines: f32,
}

impl<PerPointUserData> PointCloudBuilder<PerPointUserData>
where
    PerPointUserData: Default + Copy,
{
    pub fn new(ctx: &RenderContext) -> Self {
        const RESERVE_SIZE: usize = 512;

        // TODO(andreas): Be more resourceful about the size allocated here. Typically we know in advance!
        let color_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<Color32>(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            PointCloudDrawData::MAX_NUM_POINTS,
        );

        Self {
            vertices: Vec::with_capacity(RESERVE_SIZE),
            color_buffer,
            user_data: Vec::with_capacity(RESERVE_SIZE),
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
    pub fn batch(
        &mut self,
        label: impl Into<DebugLabel>,
    ) -> PointCloudBatchBuilder<'_, PerPointUserData> {
        self.batches.push(PointCloudBatchInfo {
            label: label.into(),
            world_from_obj: glam::Mat4::IDENTITY,
            flags: PointCloudBatchFlags::ENABLE_SHADING,
            point_count: 0,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
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

    // Iterate over all batches, yielding the batch info and a point vertex iterator zipped with its user data.
    pub fn iter_vertices_and_userdata_by_batch(
        &self,
    ) -> impl Iterator<
        Item = (
            &PointCloudBatchInfo,
            impl Iterator<Item = (&PointCloudVertex, &PerPointUserData)>,
        ),
    > {
        let mut vertex_offset = 0;
        self.batches.iter().map(move |batch| {
            let out = (
                batch,
                self.vertices
                    .iter()
                    .zip(self.user_data.iter())
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

pub struct PointCloudBatchBuilder<'a, PerPointUserData>(
    &'a mut PointCloudBuilder<PerPointUserData>,
)
where
    PerPointUserData: Default + Copy;

impl<'a, PerPointUserData> Drop for PointCloudBatchBuilder<'a, PerPointUserData>
where
    PerPointUserData: Default + Copy,
{
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().point_count == 0 {
            self.0.batches.pop();
        }
        self.extend_defaults();
    }
}

impl<'a, PerPointUserData> PointCloudBatchBuilder<'a, PerPointUserData>
where
    PerPointUserData: Default + Copy,
{
    #[inline]
    fn batch_mut(&mut self) -> &mut PointCloudBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on PointCloudBatchBuilder creation")
    }

    /// Sets the `world_from_obj` matrix for the *entire* batch.
    #[inline]
    pub fn world_from_obj(mut self, world_from_obj: glam::Mat4) -> Self {
        self.batch_mut().world_from_obj = world_from_obj;
        self
    }

    /// Sets an outline mask for every element in the batch.
    #[inline]
    pub fn outline_mask_ids(mut self, outline_mask_ids: OutlineMaskPreference) -> Self {
        self.batch_mut().overall_outline_mask_ids = outline_mask_ids;
        self
    }

    /// Each time we `add_points`, or upon builder drop, make sure that we
    /// fill in any additional colors and user-data to have matched vectors.
    fn extend_defaults(&mut self) {
        if self.0.color_buffer.num_written() < self.0.vertices.len() {
            self.0.color_buffer.extend(
                std::iter::repeat(Color32::WHITE)
                    .take(self.0.vertices.len() - self.0.color_buffer.num_written()),
            );
        }

        if self.0.user_data.len() < self.0.vertices.len() {
            self.0.user_data.extend(
                std::iter::repeat(PerPointUserData::default())
                    .take(self.0.vertices.len() - self.0.user_data.len()),
            );
        }
    }

    #[inline]
    /// Add several 3D points
    ///
    /// Returns a `PointBuilder` which can be used to set the colors, radii, and user-data for the points.
    ///
    /// Params:
    ///  - `size_hint`: The `PointBuilder` will pre-allocate buffers to accommodate up to this number of points.
    ///                 The resulting point batch, will still be determined by the length of the iterator.
    ///  - `positions`: An iterable of the positions of the collection of points
    pub fn add_points(
        &mut self,
        size_hint: usize,
        positions: impl Iterator<Item = glam::Vec3>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        // TODO(jleibs): Figure out if we can plumb-through proper support for `Iterator::size_hints()`
        // or potentially make `FixedSizedIterator` work correctly. This should be possible size the
        // underlying arrow structures are of known-size, but carries some complexity with the amount of
        // chaining, joining, filtering, etc. that happens along the way.
        crate::profile_function!();

        self.extend_defaults();

        debug_assert_eq!(self.0.vertices.len(), self.0.color_buffer.num_written());
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        let old_size = self.0.vertices.len();

        self.0.vertices.reserve(size_hint);
        self.0.vertices.extend(positions.map(|p| PointCloudVertex {
            position: p,
            radius: Size::AUTO,
        }));

        let num_points = self.0.vertices.len() - old_size;
        self.batch_mut().point_count += num_points as u32;

        self.0.user_data.reserve(num_points);

        let new_range = old_size..self.0.vertices.len();

        let max_points = self.0.vertices.len();

        PointsBuilder {
            vertices: &mut self.0.vertices[new_range],
            max_points,
            colors: &mut self.0.color_buffer,
            user_data: &mut self.0.user_data,
            additional_outline_mask_ids: &mut self
                .0
                .batches
                .last_mut()
                .unwrap()
                .additional_outline_mask_ids_vertex_ranges,
            start_vertex_index: old_size as _,
        }
    }

    #[inline]
    pub fn add_point(&mut self, position: glam::Vec3) -> PointBuilder<'_, PerPointUserData> {
        self.extend_defaults();

        debug_assert_eq!(self.0.vertices.len(), self.0.color_buffer.num_written());
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        let vertex_index = self.0.vertices.len() as u32;
        self.0.vertices.push(PointCloudVertex {
            position,
            radius: Size::AUTO,
        });
        self.0.user_data.push(Default::default());
        self.batch_mut().point_count += 1;

        PointBuilder {
            vertex: self.0.vertices.last_mut().unwrap(),
            color: &mut self.0.color_buffer,
            user_data: self.0.user_data.last_mut().unwrap(),
            vertex_index,
            additional_outline_mask_ids: &mut self
                .0
                .batches
                .last_mut()
                .unwrap()
                .additional_outline_mask_ids_vertex_ranges,
            outline_mask_id: OutlineMaskPreference::NONE,
        }
    }

    /// Adds several 2D points. Uses an autogenerated depth value, the same for all points passed.
    ///
    /// Params:
    ///  - `size_hint`: The `PointBuilder` will pre-allocate buffers to accommodate up to this number of points.
    ///                 The resulting point batch, will be the size of the length of the `positions` iterator.
    ///  - `positions`: An iterable of the positions of the collection of points
    #[inline]
    pub fn add_points_2d(
        &mut self,
        size_hint: usize,
        positions: impl Iterator<Item = glam::Vec2>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        self.add_points(size_hint, positions.map(|p| p.extend(0.0)))
    }

    /// Adds a single 2D point. Uses an autogenerated depth value.
    #[inline]
    pub fn add_point_2d(&mut self, position: glam::Vec2) -> PointBuilder<'_, PerPointUserData> {
        self.add_point(position.extend(0.0))
    }

    /// Set flags for this batch.
    pub fn flags(mut self, flags: PointCloudBatchFlags) -> Self {
        self.batch_mut().flags = flags;
        self
    }
}

// TODO(andreas): Should remove single-point builder, practically this never makes sense as we're almost always dealing with arrays of points.
pub struct PointBuilder<'a, PerPointUserData> {
    vertex: &'a mut PointCloudVertex,
    color: &'a mut CpuWriteGpuReadBuffer<Color32>,
    user_data: &'a mut PerPointUserData,
    vertex_index: u32,
    additional_outline_mask_ids: &'a mut Vec<(std::ops::Range<u32>, OutlineMaskPreference)>,
    outline_mask_id: OutlineMaskPreference,
}

impl<'a, PerPointUserData> PointBuilder<'a, PerPointUserData>
where
    PerPointUserData: Clone,
{
    #[inline]
    pub fn radius(self, radius: Size) -> Self {
        self.vertex.radius = radius;
        self
    }

    /// This mustn't call this more than once.
    #[inline]
    pub fn color(self, color: Color32) -> Self {
        self.color.push(color);
        self
    }

    pub fn user_data(self, data: PerPointUserData) -> Self {
        *self.user_data = data;
        self
    }

    /// Pushes additional outline mask ids for this point
    ///
    /// Prefer the `overall_outline_mask_ids` setting to set the outline mask ids for the entire batch whenever possible!
    pub fn outline_mask_id(mut self, outline_mask_id: OutlineMaskPreference) -> Self {
        self.outline_mask_id = outline_mask_id;
        self
    }
}

impl<'a, PerPointUserData> Drop for PointBuilder<'a, PerPointUserData> {
    fn drop(&mut self) {
        if self.outline_mask_id.is_some() {
            self.additional_outline_mask_ids.push((
                self.vertex_index..self.vertex_index + 1,
                self.outline_mask_id,
            ));
        }
    }
}

pub struct PointsBuilder<'a, PerPointUserData> {
    // Vertices is a slice, which radii will update
    vertices: &'a mut [PointCloudVertex],
    max_points: usize,
    colors: &'a mut CpuWriteGpuReadBuffer<Color32>,
    user_data: &'a mut Vec<PerPointUserData>,
    additional_outline_mask_ids: &'a mut Vec<(std::ops::Range<u32>, OutlineMaskPreference)>,
    start_vertex_index: u32,
}

impl<'a, PerPointUserData> PointsBuilder<'a, PerPointUserData>
where
    PerPointUserData: Clone,
{
    /// Assigns radii to all points.
    ///
    /// This mustn't call this more than once.
    ///
    /// If the iterator doesn't cover all points, some will not be assigned.
    /// If the iterator provides more values than there are points, the extra values will be ignored.
    #[inline]
    pub fn radii(self, radii: impl Iterator<Item = Size>) -> Self {
        // TODO(andreas): This seems like an argument for moving radius
        // to a separate storage
        crate::profile_function!();
        for (point, radius) in self.vertices.iter_mut().zip(radii) {
            point.radius = radius;
        }
        self
    }

    /// Assigns colors to all points.
    ///
    /// This mustn't call this more than once.
    ///
    /// If the iterator doesn't cover all points, some will not be assigned.
    /// If the iterator provides more values than there are points, the extra values will be ignored.
    #[inline]
    pub fn colors(self, colors: impl Iterator<Item = Color32>) -> Self {
        crate::profile_function!();
        self.colors
            .extend(colors.take(self.max_points - self.colors.num_written()));
        self
    }

    /// Assigns user data for all points in this builder.
    ///
    /// This mustn't call this more than once.
    ///
    /// User data is currently not available on the GPU.
    #[inline]
    pub fn user_data(self, data: impl Iterator<Item = PerPointUserData>) -> Self {
        crate::profile_function!();
        self.user_data
            .extend(data.take(self.max_points - self.user_data.len()));
        self
    }

    /// Pushes additional outline mask ids for a specific range of points.
    /// The range is relative to this builder's range, not the entire batch.
    ///
    /// Prefer the `overall_outline_mask_ids` setting to set the outline mask ids for the entire batch whenever possible!
    #[inline]
    pub fn push_additional_outline_mask_ids_for_range(
        self,
        range: std::ops::Range<u32>,
        ids: OutlineMaskPreference,
    ) -> Self {
        self.additional_outline_mask_ids.push((
            (range.start + self.start_vertex_index)..(range.end + self.start_vertex_index),
            ids,
        ));
        self
    }
}
