use crate::{
    renderer::{LineBatchInfo, LineDrawData, LineStripFlags, LineStripInfo, LineVertex},
    Color32, DebugLabel, Size,
};

/// Builder for a vector of line strips, making it easy to create [`crate::renderer::LineDrawData`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
#[derive(Default)]
pub struct LineStripSeriesBuilder<PerStripUserData> {
    pub vertices: Vec<LineVertex>,

    // Number of elements in strips and strip_user_data should be equal at all times.
    pub strips: Vec<LineStripInfo>,
    pub strip_user_data: Vec<PerStripUserData>,

    pub batches: Vec<LineBatchInfo>,

    /// z value given to the next 2d line strip.
    pub next_2d_z: f32,
}

impl<PerStripUserData> LineStripSeriesBuilder<PerStripUserData>
where
    PerStripUserData: Default + Copy,
{
    /// Start of a new batch.
    pub fn batch(
        &mut self,
        label: impl Into<DebugLabel>,
    ) -> LineBatchBuilder<'_, PerStripUserData> {
        self.batches.push(LineBatchInfo {
            label: label.into(),
            world_from_scene: glam::Mat4::IDENTITY,
            line_vertex_count: 0,
        });

        LineBatchBuilder(self)
    }

    /// Finalizes the builder and returns a line draw data with all the lines added so far.
    pub fn to_draw_data(&self, ctx: &mut crate::context::RenderContext) -> LineDrawData {
        LineDrawData::new(ctx, &self.vertices, &self.strips, &self.batches).unwrap()
    }

    /// Iterates over all line strips batches together with their strips and their respective vertices.
    pub fn iter_strips_with_vertices(
        &self,
    ) -> impl Iterator<
        Item = (
            (&LineStripInfo, &PerStripUserData),
            impl Iterator<Item = &LineVertex>,
        ),
    > {
        let mut cumulative_offset = 0;
        self.strips
            .iter()
            .zip(self.strip_user_data.iter())
            .enumerate()
            .map(move |(strip_index, strip)| {
                (strip, {
                    let offset = cumulative_offset;
                    let strip_index = strip_index as u32;
                    let vertex_iterator = self
                        .vertices
                        .iter()
                        .skip(offset)
                        .take_while(move |v| v.strip_index == strip_index);
                    cumulative_offset += vertex_iterator.clone().count();
                    vertex_iterator
                })
            })
    }

    pub fn is_empty(&self) -> bool {
        self.strips.is_empty()
    }
}

pub struct LineBatchBuilder<'a, PerStripUserData>(&'a mut LineStripSeriesBuilder<PerStripUserData>);

impl<'a, PerStripUserData> LineBatchBuilder<'a, PerStripUserData>
where
    PerStripUserData: Default + Copy,
{
    /// Every time we add a 2d line, we advance the z coordinate given to the next by this.
    /// We want it to be as small as possible so that if the camera shits around to 3d, things still looks like it's on a plane
    /// But if we make it too small we risk ambiguous z values (known as z fighting) under some circumstances
    const NEXT_2D_Z_STEP: f32 = -0.05;

    fn add_vertices(&mut self, points: impl Iterator<Item = glam::Vec3>, strip_index: u32) {
        let old_len = self.0.vertices.len();

        self.0
            .vertices
            .extend(points.map(|pos| LineVertex { pos, strip_index }));
        self.0.batches.last_mut().unwrap().line_vertex_count +=
            (self.0.vertices.len() - old_len) as u32;
    }

    /// Sets the `world_from_scene` matrix for the *entire* batch.
    #[inline]
    pub fn world_from_scene(&mut self, world_from_scene: glam::Mat4) -> &mut Self {
        self.0.batches.last_mut().unwrap().world_from_scene = world_from_scene;
        self
    }

    /// Adds a 3D series of line connected points.
    pub fn add_strip(
        &mut self,
        points: impl Iterator<Item = glam::Vec3>,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let old_len = self.0.strips.len();
        let strip_index = old_len as _;

        self.add_vertices(points, strip_index);

        debug_assert_eq!(self.0.strips.len(), self.0.strip_user_data.len());
        self.0.strips.push(LineStripInfo::default());
        self.0.strip_user_data.push(PerStripUserData::default());

        LineStripBuilder {
            strips: &mut self.0.strips[old_len..],
            user_data: &mut self.0.strip_user_data[old_len..],
        }
    }

    /// Adds a single 3D line segment connecting two points.
    #[inline]
    pub fn add_segment(
        &mut self,
        a: glam::Vec3,
        b: glam::Vec3,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        self.add_strip([a, b].into_iter())
    }

    /// Adds a series of unconnected 3D line segments.
    pub fn add_segments(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec3, glam::Vec3)>,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let mut num_strips = self.0.strips.len() as u32;

        // It's tempting to assign the same strip to all vertices, after all they share
        // color/radius/tag properties.
        // However, if we don't assign different strip indices, we don't know when a strip (==segment) starts and ends.
        for (a, b) in segments {
            self.add_vertices([a, b].into_iter(), num_strips);
            num_strips += 1;
        }

        let old_len = self.0.strips.len();
        let num_strips_added = num_strips as usize - old_len;

        debug_assert_eq!(self.0.strips.len(), self.0.strip_user_data.len());
        self.0
            .strips
            .extend(std::iter::repeat(LineStripInfo::default()).take(num_strips_added));
        self.0
            .strip_user_data
            .extend(std::iter::repeat(PerStripUserData::default()).take(num_strips_added));

        LineStripBuilder {
            strips: &mut self.0.strips[old_len..],
            user_data: &mut self.0.strip_user_data[old_len..],
        }
    }

    /// Add rectangle outlines.
    ///
    /// Internally adds 4 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_rectangle_outline(
        &mut self,
        top_left_corner: glam::Vec3,
        extent_u: glam::Vec3,
        extent_v: glam::Vec3,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        self.add_segments(
            [
                (top_left_corner, top_left_corner + extent_u),
                (
                    top_left_corner + extent_u,
                    top_left_corner + extent_u + extent_v,
                ),
                (
                    top_left_corner + extent_u + extent_v,
                    top_left_corner + extent_v,
                ),
                (top_left_corner + extent_v, top_left_corner),
            ]
            .into_iter(),
        )
        .flags(
            LineStripFlags::CAP_END_ROUND
                | LineStripFlags::CAP_START_ROUND
                | LineStripFlags::NO_COLOR_GRADIENT,
        )
    }

    /// Adds a 2D series of line connected points.
    ///
    /// Uses autogenerated depth value.
    #[inline]
    pub fn add_strip_2d(
        &mut self,
        points: impl Iterator<Item = glam::Vec2>,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_strip(points.map(|p| p.extend(z)))
    }

    /// Adds a single 2D line segment connecting two points. Uses autogenerated depth value.
    #[inline]
    pub fn add_segment_2d(
        &mut self,
        a: glam::Vec2,
        b: glam::Vec2,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        self.add_strip_2d([a, b].into_iter())
    }

    /// Adds a series of unconnected 2D line segments.
    ///
    /// Uses autogenerated depth value, all segments get the same depth value.
    #[inline]
    pub fn add_segments_2d(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec2, glam::Vec2)>,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_segments(segments.map(|(a, b)| (a.extend(z), b.extend(z))))
    }

    /// Add 2D rectangle outlines.
    ///
    /// Internally adds 4 2D line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_rectangle_outline_2d(
        &mut self,
        top_left_corner: glam::Vec2,
        extent_u: glam::Vec2,
        extent_v: glam::Vec2,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_rectangle_outline(
            top_left_corner.extend(z),
            extent_u.extend(0.0),
            extent_v.extend(0.0),
        )
    }

    /// Add 2D rectangle outlines with axis along X and Y.
    ///
    /// Internally adds 4 2D line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_axis_aligned_rectangle_outline_2d(
        &mut self,
        min: glam::Vec2,
        max: glam::Vec2,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let z = self.0.next_2d_z;
        self.0.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_rectangle_outline(
            min.extend(z),
            glam::Vec3::X * (max.x - min.x),
            glam::Vec3::Y * (max.y - min.y),
        )
    }
}

pub struct LineStripBuilder<'a, PerStripUserData> {
    strips: &'a mut [LineStripInfo],
    user_data: &'a mut [PerStripUserData],
}

impl<'a, PerStripUserData> LineStripBuilder<'a, PerStripUserData>
where
    PerStripUserData: Clone,
{
    #[inline]
    pub fn radius(self, radius: Size) -> Self {
        for strip in self.strips.iter_mut() {
            strip.radius = radius;
        }
        self
    }

    #[inline]
    pub fn color(self, color: Color32) -> Self {
        for strip in self.strips.iter_mut() {
            strip.color = color;
        }
        self
    }

    #[inline]
    pub fn flags(self, flags: LineStripFlags) -> Self {
        for strip in self.strips.iter_mut() {
            strip.flags = flags;
        }
        self
    }

    /// Adds user data for every strip this builder adds.
    ///
    /// User data is currently not available on the GPU.
    #[inline]
    pub fn user_data(self, user_data: PerStripUserData) -> Self {
        for d in self.user_data.iter_mut() {
            *d = user_data.clone();
        }
        self
    }
}
