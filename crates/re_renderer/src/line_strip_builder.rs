use std::ops::Range;

use crate::{
    renderer::{
        LineBatchInfo, LineDrawData, LineStripFlags, LineStripInfo, LineVertex,
        OutlineMaskPreference,
    },
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
            world_from_obj: glam::Mat4::IDENTITY,
            line_vertex_count: 0,
            overall_outline_mask: OutlineMaskPreference::NONE,
            additional_outline_mask_vertex_ranges: Vec::new(),
        });

        LineBatchBuilder(self)
    }

    // Iterate over all batches, yielding the batch info and all line vertices (note that these will span several line strips!)
    pub fn iter_vertices_by_batch(
        &self,
    ) -> impl Iterator<Item = (&LineBatchInfo, impl Iterator<Item = &LineVertex>)> {
        let mut vertex_offset = 0;
        self.batches.iter().map(move |batch| {
            let out = (
                batch,
                self.vertices
                    .iter()
                    .skip(vertex_offset)
                    .take(batch.line_vertex_count as usize),
            );
            vertex_offset += batch.line_vertex_count as usize;
            out
        })
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

impl<'a, PerStripUserData> Drop for LineBatchBuilder<'a, PerStripUserData> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().line_vertex_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl<'a, PerStripUserData> LineBatchBuilder<'a, PerStripUserData>
where
    PerStripUserData: Default + Copy,
{
    #[inline]
    fn batch_mut(&mut self) -> &mut LineBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on PointCloudBatchBuilder creation")
    }

    fn add_vertices(&mut self, points: impl Iterator<Item = glam::Vec3>, strip_index: u32) {
        let old_len = self.0.vertices.len();

        self.0.vertices.extend(points.map(|pos| LineVertex {
            position: pos,
            strip_index,
        }));
        self.batch_mut().line_vertex_count += (self.0.vertices.len() - old_len) as u32;
    }

    /// Sets the `world_from_obj` matrix for the *entire* batch.
    #[inline]
    pub fn world_from_obj(mut self, world_from_obj: glam::Mat4) -> Self {
        self.batch_mut().world_from_obj = world_from_obj;
        self
    }

    /// Sets an outline mask for every element in the batch.
    #[inline]
    pub fn outline_mask(mut self, outline_mask: OutlineMaskPreference) -> Self {
        self.batch_mut().overall_outline_mask = outline_mask;
        self
    }

    /// Adds a 3D series of line connected points.
    pub fn add_strip(
        &mut self,
        points: impl Iterator<Item = glam::Vec3>,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let old_strip_count = self.0.strips.len();
        let old_vertex_count = self.0.vertices.len();
        let strip_index = old_strip_count as _;

        self.add_vertices(points, strip_index);
        let new_vertex_count = self.0.vertices.len();

        debug_assert_eq!(self.0.strips.len(), self.0.strip_user_data.len());
        self.0.strips.push(LineStripInfo::default());
        self.0.strip_user_data.push(PerStripUserData::default());
        let new_strip_count = self.0.strips.len();

        LineStripBuilder {
            builder: self.0,
            outline_mask: OutlineMaskPreference::NONE,
            vertex_range: old_vertex_count..new_vertex_count,
            strip_range: old_strip_count..new_strip_count,
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
        let old_strip_count = self.0.strips.len();
        let old_vertex_count = self.0.vertices.len();
        let mut strip_index = old_strip_count as u32;

        // It's tempting to assign the same strip to all vertices, after all they share
        // color/radius/tag properties.
        // However, if we don't assign different strip indices, we don't know when a strip (==segment) starts and ends.
        for (a, b) in segments {
            self.add_vertices([a, b].into_iter(), strip_index);
            strip_index += 1;
        }
        let new_vertex_count = self.0.vertices.len();
        let num_strips_added = strip_index as usize - old_strip_count;

        debug_assert_eq!(self.0.strips.len(), self.0.strip_user_data.len());
        self.0
            .strips
            .extend(std::iter::repeat(LineStripInfo::default()).take(num_strips_added));
        self.0
            .strip_user_data
            .extend(std::iter::repeat(PerStripUserData::default()).take(num_strips_added));
        let new_strip_count = self.0.strips.len();

        LineStripBuilder {
            builder: self.0,
            outline_mask: OutlineMaskPreference::NONE,
            vertex_range: old_vertex_count..new_vertex_count,
            strip_range: old_strip_count..new_strip_count,
        }
    }

    /// Add box outlines from a unit cube transformed by `transform`.
    ///
    /// Internally adds 12 line segments with rounded line heads.
    /// Disables color gradient since we don't support gradients in this setup yet (i.e. enabling them does not look good)
    #[inline]
    pub fn add_box_outline(
        &mut self,
        transform: glam::Affine3A,
    ) -> LineStripBuilder<'_, PerStripUserData> {
        let corners = [
            transform.transform_point3(glam::vec3(-0.5, -0.5, -0.5)),
            transform.transform_point3(glam::vec3(-0.5, -0.5, 0.5)),
            transform.transform_point3(glam::vec3(-0.5, 0.5, -0.5)),
            transform.transform_point3(glam::vec3(-0.5, 0.5, 0.5)),
            transform.transform_point3(glam::vec3(0.5, -0.5, -0.5)),
            transform.transform_point3(glam::vec3(0.5, -0.5, 0.5)),
            transform.transform_point3(glam::vec3(0.5, 0.5, -0.5)),
            transform.transform_point3(glam::vec3(0.5, 0.5, 0.5)),
        ];
        self.add_segments(
            [
                // bottom:
                (corners[0b000], corners[0b001]),
                (corners[0b000], corners[0b010]),
                (corners[0b011], corners[0b001]),
                (corners[0b011], corners[0b010]),
                // top:
                (corners[0b100], corners[0b101]),
                (corners[0b100], corners[0b110]),
                (corners[0b111], corners[0b101]),
                (corners[0b111], corners[0b110]),
                // sides:
                (corners[0b000], corners[0b100]),
                (corners[0b001], corners[0b101]),
                (corners[0b010], corners[0b110]),
                (corners[0b011], corners[0b111]),
            ]
            .into_iter(),
        )
        .flags(
            LineStripFlags::CAP_END_ROUND
                | LineStripFlags::CAP_START_ROUND
                | LineStripFlags::NO_COLOR_GRADIENT,
        )
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
        self.add_strip(points.map(|p| p.extend(0.0)))
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
        self.add_segments(segments.map(|(a, b)| (a.extend(0.0), b.extend(0.0))))
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
        self.add_rectangle_outline(
            top_left_corner.extend(0.0),
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
        self.add_rectangle_outline(
            min.extend(0.0),
            glam::Vec3::X * (max.x - min.x),
            glam::Vec3::Y * (max.y - min.y),
        )
    }
}

pub struct LineStripBuilder<'a, PerStripUserData> {
    builder: &'a mut LineStripSeriesBuilder<PerStripUserData>,
    outline_mask: OutlineMaskPreference,
    vertex_range: Range<usize>,
    strip_range: Range<usize>,
}

impl<'a, PerStripUserData> LineStripBuilder<'a, PerStripUserData>
where
    PerStripUserData: Clone,
{
    #[inline]
    pub fn radius(self, radius: Size) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.radius = radius;
        }
        self
    }

    #[inline]
    pub fn color(self, color: Color32) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.color = color;
        }
        self
    }

    #[inline]
    pub fn flags(self, flags: LineStripFlags) -> Self {
        for strip in self.builder.strips[self.strip_range.clone()].iter_mut() {
            strip.flags = flags;
        }
        self
    }

    /// Sets an individual outline mask.
    /// Note that this has a relatively high performance impact.
    #[inline]
    pub fn outline_mask(mut self, outline_mask: OutlineMaskPreference) -> Self {
        self.outline_mask = outline_mask;
        self
    }

    /// Adds user data for every strip this builder adds.
    ///
    /// User data is currently not available on the GPU.
    #[inline]
    pub fn user_data(self, user_data: PerStripUserData) -> Self {
        for d in self.builder.strip_user_data[self.strip_range.clone()].iter_mut() {
            *d = user_data.clone();
        }
        self
    }
}

impl<'a, PerStripUserData> Drop for LineStripBuilder<'a, PerStripUserData> {
    fn drop(&mut self) {
        if self.outline_mask.is_some() {
            self.builder
                .batches
                .last_mut()
                .unwrap()
                .additional_outline_mask_vertex_ranges
                .push((
                    self.vertex_range.start as u32..self.vertex_range.end as u32,
                    self.outline_mask,
                ));
        }
    }
}
