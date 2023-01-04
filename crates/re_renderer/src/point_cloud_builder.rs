use crate::{
    renderer::{
        PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError, PointCloudVertex,
    },
    staging_write_belt::{StagingWriteBeltBuffer, StagingWriteBeltBufferTypedView},
    Color32, DebugLabel, RenderContext, Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
pub struct PointCloudBuilder<PerPointUserData> {
    // Size of `point`/color`/`per_point_user_data` must be equal.
    // TODO(andreas): Now that we're feeding write-only gpu buffers, should this really have the responsibility of storing cpu data?
    pub vertices: Vec<PointCloudVertex>,
    pub user_data: Vec<PerPointUserData>,

    pub(crate) batches: Vec<PointCloudBatchInfo>,

    pub(crate) vertices_gpu: StagingWriteBeltBuffer,
    pub(crate) colors_gpu: StagingWriteBeltBuffer,

    /// z value given to the next 2d point.
    pub next_2d_z: f32,
}

impl<PerPointUserData> PointCloudBuilder<PerPointUserData>
where
    PerPointUserData: Default + Clone,
{
    pub fn new(ctx: &mut RenderContext, max_num_points: usize, max_num_batches: usize) -> Self {
        // TODO: check max_num_points bound

        let mut staging_belt = ctx.staging_belt.lock();
        let vertices_gpu = staging_belt.allocate(
            &ctx.device,
            &mut ctx.gpu_resources.buffers,
            (std::mem::size_of::<PointCloudVertex>() * max_num_points) as wgpu::BufferAddress,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64,
        );
        let mut colors_gpu = staging_belt.allocate(
            &ctx.device,
            &mut ctx.gpu_resources.buffers,
            (std::mem::size_of::<PointCloudVertex>() * max_num_points) as wgpu::BufferAddress,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64,
        );
        // Default unassigned colors to white.
        // TODO(andreas): Do we actually need this? Can we do this lazily if no color was specified?
        colors_gpu.memset(255);

        Self {
            vertices: Vec::with_capacity(max_num_points),
            batches: Vec::with_capacity(max_num_batches),
            user_data: Vec::with_capacity(max_num_points),

            vertices_gpu,
            colors_gpu,

            next_2d_z: 0.0,
        }
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
            point_count: 0,
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
                    .skip(vertex_offset),
            );
            vertex_offset += batch.point_count as usize;
            out
        })
    }

    /// Finalizes the builder and returns a point cloud draw data with all the added points.
    pub fn to_draw_data(
        self,
        ctx: &mut crate::context::RenderContext,
    ) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(ctx, self)
    }
}

pub struct PointCloudBatchBuilder<'a, PerPointUserData>(
    &'a mut PointCloudBuilder<PerPointUserData>,
);

impl<'a, PerPointUserData> PointCloudBatchBuilder<'a, PerPointUserData>
where
    PerPointUserData: Default + Copy,
{
    /// Every time we add a 2d point, we advance the z coordinate given to the next by this.
    /// We want it to be as small as possible so that if the camera shifts around to 3d, things still looks like it's on a plane
    /// But if we make it too small we risk ambiguous z values (known as z fighting) under some circumstances
    const NEXT_2D_Z_STEP: f32 = -0.05;

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

    #[inline]
    pub fn add_vertices(
        &mut self,
        vertices: impl Iterator<Item = PointCloudVertex>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        let old_size = self.0.vertices.len();

        self.0.vertices.extend(vertices);

        let num_points = self.0.vertices.len() - old_size;
        self.batch_mut().point_count += num_points as u32;

        self.0
            .user_data
            .extend(std::iter::repeat(PerPointUserData::default()).take(num_points));

        let new_range = old_size..self.0.vertices.len();

        PointsBuilder {
            vertices: &mut self.0.vertices[new_range.clone()],
            vertices_gpu: self.0.vertices_gpu.typed_view(old_size),
            colors_gpu: self.0.colors_gpu.typed_view(old_size),
            user_data: &mut self.0.user_data[new_range],
        }
    }

    #[inline]
    pub fn add_points(
        &mut self,
        positions: impl Iterator<Item = glam::Vec3>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        self.add_vertices(positions.map(|p| PointCloudVertex {
            position: p,
            radius: Size::AUTO,
        }))
    }

    #[inline]
    pub fn add_point(&mut self, position: glam::Vec3) -> PointBuilder<'_, PerPointUserData> {
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        let old_size = self.0.vertices.len();
        let vertex = PointCloudVertex {
            position,
            radius: Size::AUTO,
        };
        self.0.vertices.push(vertex);

        self.0.user_data.push(PerPointUserData::default());
        self.batch_mut().point_count += 1;

        PointBuilder {
            vertex: self.0.vertices.last_mut().unwrap(),
            vertices_gpu: self.0.vertices_gpu.typed_view(old_size),
            colors_gpu: self.0.colors_gpu.typed_view(old_size),
            user_data: self.0.user_data.last_mut().unwrap(),
        }
    }

    /// Adds several 2D points. Uses an autogenerated depth value, the same for all points passed.
    #[inline]
    pub fn add_points_2d(
        &mut self,
        positions: impl Iterator<Item = glam::Vec2>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_points(positions.map(|p| p.extend(z)))
    }

    /// Adds a single 2D point. Uses an autogenerated depth value.
    #[inline]
    pub fn add_point_2d(&mut self, position: glam::Vec2) -> PointBuilder<'_, PerPointUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_point(position.extend(z))
    }
}

pub struct PointBuilder<'a, PerPointUserData> {
    vertex: &'a mut PointCloudVertex,
    colors_gpu: StagingWriteBeltBufferTypedView<'a, Color32>,
    vertices_gpu: StagingWriteBeltBufferTypedView<'a, PointCloudVertex>,
    user_data: &'a mut PerPointUserData,
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

    #[inline]
    pub fn color(mut self, color: Color32) -> Self {
        self.colors_gpu.write_single(&color, 0);
        self
    }

    pub fn user_data(self, data: PerPointUserData) -> Self {
        *self.user_data = data;
        self
    }
}

impl<'a, PerPointUserData> Drop for PointBuilder<'a, PerPointUserData> {
    #[inline]
    fn drop(&mut self) {
        self.vertices_gpu.write_single(self.vertex, 0);
    }
}

pub struct PointsBuilder<'a, PerPointUserData> {
    vertices: &'a mut [PointCloudVertex],
    colors_gpu: StagingWriteBeltBufferTypedView<'a, Color32>,
    vertices_gpu: StagingWriteBeltBufferTypedView<'a, PointCloudVertex>,
    user_data: &'a mut [PerPointUserData],
}

impl<'a, PerPointUserData> PointsBuilder<'a, PerPointUserData>
where
    PerPointUserData: Clone,
{
    /// Splats a radius to all points in this builder.
    #[inline]
    pub fn radius(self, radius: Size) -> Self {
        for point in self.vertices.iter_mut() {
            point.radius = radius;
        }
        self
    }

    /// Assigns radii to all points.
    ///
    /// If the iterator doesn't cover all points, some will not be assigned.
    /// If the iterator provides more values than there are points, the extra values will be ignored.
    #[inline]
    pub fn radii(self, radii: impl Iterator<Item = Size>) -> Self {
        for (point, radius) in self.vertices.iter_mut().zip(radii) {
            point.radius = radius;
        }
        self
    }

    /// Splats a color to all points in this builder.
    #[inline]
    pub fn color(mut self, color: Color32) -> Self {
        self.colors_gpu.fill(&color);
        self
    }

    /// Assigns colors to all points.
    ///
    /// The slice is required to cover all points.
    #[inline]
    pub fn colors(mut self, colors: impl Iterator<Item = Color32>) -> Self {
        self.colors_gpu.write(colors, 0);
        self
    }

    /// Splats a single user data to all points in this builder.
    ///
    /// User data is currently not available on the GPU.
    #[inline]
    pub fn user_data_splat(self, data: PerPointUserData) -> Self {
        for user_data in self.user_data.iter_mut() {
            *user_data = data.clone();
        }
        self
    }

    /// Assigns user data for all points in this builder.
    ///
    /// User data is currently not available on the GPU.
    #[inline]
    pub fn user_data(self, data: impl Iterator<Item = PerPointUserData>) -> Self {
        for (d, data) in self.user_data.iter_mut().zip(data) {
            *d = data;
        }
        self
    }
}

impl<'a, PerPointUserData> Drop for PointsBuilder<'a, PerPointUserData> {
    #[inline]
    fn drop(&mut self) {
        self.vertices_gpu.write(self.vertices.iter().cloned(), 0);
    }
}
