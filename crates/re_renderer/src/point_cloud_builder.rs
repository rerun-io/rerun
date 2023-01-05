use crate::{
    renderer::{
        PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError, PointCloudVertex,
    },
    Color32, DebugLabel, Size,
};

/// Builder for point clouds, making it easy to create [`crate::renderer::PointCloudDrawData`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
pub struct PointCloudBuilder<PerPointUserData> {
    // Size of `point`/color`/`per_point_user_data` must be equal.
    pub vertices: Vec<PointCloudVertex>,
    pub colors: Vec<Color32>,
    pub user_data: Vec<PerPointUserData>,

    batches: Vec<PointCloudBatchInfo>,
}

impl<PerPointUserData> Default for PointCloudBuilder<PerPointUserData> {
    fn default() -> Self {
        const RESERVE_SIZE: usize = 512;
        Self {
            vertices: Vec::with_capacity(RESERVE_SIZE),
            colors: Vec::with_capacity(RESERVE_SIZE),
            user_data: Vec::with_capacity(RESERVE_SIZE),
            batches: Vec::with_capacity(16),
        }
    }
}

impl<PerPointUserData> PointCloudBuilder<PerPointUserData>
where
    PerPointUserData: Default + Clone,
{
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
                    .skip(vertex_offset)
                    .take(batch.point_count as usize),
            );
            vertex_offset += batch.point_count as usize;
            out
        })
    }

    /// Finalizes the builder and returns a point cloud draw data with all the points added so far.
    pub fn to_draw_data(
        &self,
        ctx: &mut crate::context::RenderContext,
    ) -> Result<PointCloudDrawData, PointCloudDrawDataError> {
        PointCloudDrawData::new(ctx, &self.vertices, &self.colors, &self.batches)
    }
}

pub struct PointCloudBatchBuilder<'a, PerPointUserData>(
    &'a mut PointCloudBuilder<PerPointUserData>,
);

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

    #[inline]
    pub fn add_points(
        &mut self,
        positions: impl Iterator<Item = glam::Vec3>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        debug_assert_eq!(self.0.vertices.len(), self.0.colors.len());
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        let old_size = self.0.vertices.len();

        self.0.vertices.extend(positions.map(|p| PointCloudVertex {
            position: p,
            radius: Size::AUTO,
        }));

        let num_points = self.0.vertices.len() - old_size;
        self.batch_mut().point_count += num_points as u32;

        self.0
            .colors
            .extend(std::iter::repeat(Color32::WHITE).take(num_points));
        self.0
            .user_data
            .extend(std::iter::repeat(PerPointUserData::default()).take(num_points));

        let new_range = old_size..self.0.vertices.len();

        PointsBuilder {
            vertices: &mut self.0.vertices[new_range.clone()],
            colors: &mut self.0.colors[new_range.clone()],
            user_data: &mut self.0.user_data[new_range],
        }
    }

    #[inline]
    pub fn add_point(&mut self, position: glam::Vec3) -> PointBuilder<'_, PerPointUserData> {
        debug_assert_eq!(self.0.vertices.len(), self.0.colors.len());
        debug_assert_eq!(self.0.vertices.len(), self.0.user_data.len());

        self.0.vertices.push(PointCloudVertex {
            position,
            radius: Size::AUTO,
        });
        self.0.colors.push(Color32::WHITE);
        self.0.user_data.push(PerPointUserData::default());
        self.batch_mut().point_count += 1;

        PointBuilder {
            vertex: self.0.vertices.last_mut().unwrap(),
            color: self.0.colors.last_mut().unwrap(),
            user_data: self.0.user_data.last_mut().unwrap(),
        }
    }

    /// Adds several 2D points. Uses an autogenerated depth value, the same for all points passed.
    #[inline]
    pub fn add_points_2d(
        &mut self,
        positions: impl Iterator<Item = glam::Vec2>,
    ) -> PointsBuilder<'_, PerPointUserData> {
        self.add_points(positions.map(|p| p.extend(0.0)))
    }

    /// Adds a single 2D point. Uses an autogenerated depth value.
    #[inline]
    pub fn add_point_2d(&mut self, position: glam::Vec2) -> PointBuilder<'_, PerPointUserData> {
        self.add_point(position.extend(0.0))
    }
}

pub struct PointBuilder<'a, PerPointUserData> {
    vertex: &'a mut PointCloudVertex,
    color: &'a mut Color32,
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
    pub fn color(self, color: Color32) -> Self {
        *self.color = color;
        self
    }

    pub fn user_data(self, data: PerPointUserData) -> Self {
        *self.user_data = data;
        self
    }
}

pub struct PointsBuilder<'a, PerPointUserData> {
    vertices: &'a mut [PointCloudVertex],
    colors: &'a mut [Color32],
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
    pub fn color(self, color: Color32) -> Self {
        for c in self.colors.iter_mut() {
            *c = color;
        }
        self
    }

    /// Assigns colors to all points.
    ///
    /// If the iterator doesn't cover all points, some will not be assigned.
    /// If the iterator provides more values than there are points, the extra values will be ignored.
    #[inline]
    pub fn colors(self, colors: impl Iterator<Item = Color32>) -> Self {
        for (c, color) in self.colors.iter_mut().zip(colors) {
            *c = color;
        }
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
