use crate::renderer::{LineDrawable, LineStrip, LineStripFlags};

/// Builder for a vector of line strips, making it easy to create [`crate::renderer::LineDrawable`].
///
/// Among other things helps with consistent 2D ordering.
///
/// TODO(andreas): We could make significant optimizations here by making this builder capable
/// of writing to a GPU readable memory location.
/// This will require some ahead of time size limit, but should be feasible.
/// But before that we first need to sort out cpu->gpu transfers better by providing staging buffers.
#[derive(Default)]
pub struct LineStripSeriesBuilder {
    pub strips: Vec<LineStrip>,
}

impl LineStripSeriesBuilder {
    /// Adds a 3D series of line connected points.
    pub fn add_strip(&mut self, points: impl Iterator<Item = glam::Vec3>) -> LineStripBuilder<'_> {
        let old_len = self.strips.len();
        self.strips.push(LineStrip {
            points: points.collect(),
            ..Default::default()
        });
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
        let old_len = self.strips.len();
        for (a, b) in segments {
            self.add_strip([a, b].into_iter());
        }
        LineStripBuilder(&mut self.strips[old_len..])
    }

    /// Finalizes the builder and returns a line drawable with all the lines added so far.
    pub fn to_drawable(&self, ctx: &mut crate::context::RenderContext) -> LineDrawable {
        LineDrawable::new(ctx, &self.strips).unwrap()
    }
}

pub struct LineStripBuilder<'a>(&'a mut [LineStrip]);

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

    pub fn color_rgba_slice(self, rgba: [u8; 4]) -> Self {
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
