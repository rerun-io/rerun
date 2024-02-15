use crate::{renderer::LineDrawDataError, LineStripBatchBuilder, QueueableDrawData, RenderContext};

/// Simple allocator mechanism to manage line strip builders.
///
/// It allows you to lazily create line strip builders if the need arises and close full ones.
/// Use this only if you don't know the number of strips and vertices ahead of time,
/// otherwise use [`LineStripBatchBuilder`] directly!
///
/// Creating new line strip builders is fairly expensive and should be avoided if possible!
pub struct LineStripBatchBuilderAllocator {
    active_line_builder: Option<LineStripBatchBuilder>,
    draw_data: Vec<QueueableDrawData>,

    min_num_strips: u32,
    min_num_vertices: u32,
}

impl LineStripBatchBuilderAllocator {
    pub fn new(min_num_strips: u32, min_num_vertices: u32) -> Self {
        // The internal data texture rows are aligned to 256 bytes.
        // A single line strip takes 8 bytes, making 32 strips the lowest meaningful minimum.
        // A single line vertex takes 16 bytes, making 16 vertices the lowest meaningful minimum.
        const MIN_NUM_STRIPS: u32 = 32;
        const MIN_NUM_VERTICES: u32 = 16;

        Self {
            active_line_builder: None,
            draw_data: Vec::new(),

            min_num_strips: min_num_strips.max(MIN_NUM_STRIPS),
            min_num_vertices: min_num_vertices.max(MIN_NUM_VERTICES),
        }
    }

    /// Returns a line strip builder that is guaranteed to have at least the given amount of space.
    pub fn reserve<'a>(
        &'a mut self,
        render_ctx: &RenderContext,
        num_strips: u32,
        num_vertices: u32,
    ) -> Result<&'a LineStripBatchBuilder, LineDrawDataError> {
        if self.active_line_builder.as_ref().map_or(false, |b| {
            b.remaining_strip_capacity() >= num_strips
                && b.remaining_vertex_capacity() >= num_vertices
        }) {
            if let Some(line_builder) = self.active_line_builder.take() {
                self.draw_data
                    .push(line_builder.into_draw_data(render_ctx)?.into());
            }
        }

        if self.active_line_builder.is_none() {
            self.active_line_builder = Some(LineStripBatchBuilder::new(
                render_ctx,
                num_strips.min(self.min_num_strips),
                num_vertices.min(self.min_num_vertices),
            ));
        };

        Ok(self.active_line_builder.as_mut().unwrap())
    }

    pub fn finish(
        mut self,
        render_ctx: &RenderContext,
    ) -> Result<Vec<QueueableDrawData>, LineDrawDataError> {
        if let Some(line_builder) = self.active_line_builder {
            self.draw_data
                .push(line_builder.into_draw_data(render_ctx)?.into());
        }
        Ok(self.draw_data)
    }
}
