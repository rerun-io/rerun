use crate::renderer::{LineDrawable, LineStripFlags, LineStripInfo, LineVertex};

/// Builder for a vector of line strips, making it easy to create [`crate::renderer::LineDrawable`].
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
#[derive(Default)]
pub struct LineStripSeriesBuilder {
    pub strips: Vec<LineStripInfo>,
    pub vertices: Vec<LineVertex>,

    /// z value given to the next 2d line strip.
    next_2d_z: f32,
}

impl LineStripSeriesBuilder {
    /// Every time we add a 2d line, we advance the z coordinate given to the next by this.
    /// We want it to be as small as possible so that if the camera shits around to 3d, things still looks like it's on a plane
    /// But if we make it too small we risk ambiguous z values (known as z fighting) under some circumstances
    const NEXT_2D_Z_STEP: f32 = -0.05;

    /// Adds a 3D series of line connected points.
    pub fn add_strip(&mut self, points: impl Iterator<Item = glam::Vec3>) -> LineStripBuilder<'_> {
        let old_len = self.strips.len();
        let strip_index = old_len as _;
        self.vertices
            .extend(points.map(|pos| LineVertex { pos, strip_index }));
        self.strips.push(LineStripInfo::default());
        LineStripBuilder(&mut self.strips[old_len..])
    }

    /// Adds a single 3D line segment connecting two points.
    pub fn add_segment(&mut self, a: glam::Vec3, b: glam::Vec3) -> LineStripBuilder<'_> {
        self.add_strip([a, b].into_iter())
    }

    /// Adds a series of unconnected 3D line segments.
    pub fn add_segments(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec3, glam::Vec3)>,
    ) -> LineStripBuilder<'_> {
        let mut num_strips = self.strips.len() as u32;

        // It's tempting to assign the same strip to all vertices, after all they share
        // color/radius/tag properties.
        // However, if we don't assign different strip indices, we don't know when a strip (==segment) starts and ends.
        for (a, b) in segments {
            self.vertices.extend(
                [
                    LineVertex {
                        pos: a,
                        strip_index: num_strips,
                    },
                    LineVertex {
                        pos: b,
                        strip_index: num_strips,
                    },
                ]
                .into_iter(),
            );
            num_strips += 1;
        }

        let old_len = self.strips.len();
        self.strips.extend(
            std::iter::repeat(LineStripInfo::default()).take(num_strips as usize - old_len),
        );
        LineStripBuilder(&mut self.strips[old_len..])
    }

    /// Adds a 2D series of line connected points.
    ///
    /// Uses autogenerated depth value.
    pub fn add_strip_2d(
        &mut self,
        points: impl Iterator<Item = glam::Vec2>,
    ) -> LineStripBuilder<'_> {
        let z = self.next_2d_z;
        self.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_strip(points.map(|p| p.extend(z)))
    }

    /// Adds a single 2D line segment connecting two points. Uses autogenerated depth value.
    pub fn add_segment_2d(&mut self, a: glam::Vec2, b: glam::Vec2) -> LineStripBuilder<'_> {
        self.add_strip_2d([a, b].into_iter())
    }

    /// Adds a series of unconnected 2D line segments.
    ///
    /// Uses autogenerated depth value, all segments get the same depth value.
    pub fn add_segments_2d(
        &mut self,
        segments: impl Iterator<Item = (glam::Vec2, glam::Vec2)>,
    ) -> LineStripBuilder<'_> {
        let z = self.next_2d_z;
        self.next_2d_z += Self::NEXT_2D_Z_STEP;
        self.add_segments(segments.map(|(a, b)| (a.extend(z), b.extend(z))))
    }

    /// Finalizes the builder and returns a line drawable with all the lines added so far.
    pub fn to_drawable(&self, ctx: &mut crate::context::RenderContext) -> LineDrawable {
        LineDrawable::new(ctx, &self.vertices, &self.strips).unwrap()
    }

    /// Iterates over all line strips together with their respective vertices.
    pub fn iter_strips_mut_with_vertices(
        &mut self,
    ) -> impl Iterator<Item = (&mut LineStripInfo, impl Iterator<Item = &LineVertex>)> {
        Self::iter_strips_with_vertices_internal(self.strips.iter_mut(), &self.vertices)
    }

    /// Iterates over all line strips together with their respective vertices.
    pub fn iter_strips_with_vertices(
        &self,
    ) -> impl Iterator<Item = (&LineStripInfo, impl Iterator<Item = &LineVertex>)> {
        Self::iter_strips_with_vertices_internal(self.strips.iter(), &self.vertices)
    }

    fn iter_strips_with_vertices_internal<S>(
        strip_iter: impl Iterator<Item = S>,
        vertices: &[LineVertex],
    ) -> impl Iterator<Item = (S, impl Iterator<Item = &LineVertex>)> {
        let mut cumulative_offset = 0;
        strip_iter.enumerate().map(move |(strip_index, strip)| {
            (strip, {
                let offset = cumulative_offset;
                let strip_index = strip_index as u32;
                let vertex_iterator = vertices
                    .iter()
                    .skip(offset)
                    .take_while(move |v| v.strip_index == strip_index);
                cumulative_offset += vertex_iterator.clone().count();
                vertex_iterator
            })
        })
    }
}

pub struct LineStripBuilder<'a>(&'a mut [LineStripInfo]);

impl<'a> LineStripBuilder<'a> {
    pub fn radius(self, radius: f32) -> Self {
        for strip in self.0.iter_mut() {
            strip.radius = radius;
        }
        self
    }

    pub fn color_rgb(self, r: u8, g: u8, b: u8) -> Self {
        for strip in self.0.iter_mut() {
            strip.srgb_color[0] = r;
            strip.srgb_color[1] = g;
            strip.srgb_color[2] = b;
        }
        self
    }

    pub fn color_rgbx_slice(self, rgba: [u8; 4]) -> Self {
        for strip in self.0.iter_mut() {
            strip.srgb_color = rgba;
        }
        self
    }

    pub fn flags(self, flags: LineStripFlags) -> Self {
        for strip in self.0.iter_mut() {
            strip.flags = flags;
        }
        self
    }
}
