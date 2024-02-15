use crate::{renderer::LineDrawDataError, LineDrawableBuilder, QueueableDrawData, RenderContext};

/// Simple allocator mechanism to manage line strip builders.
///
/// It allows you to lazily create line strip builders if the need arises and close full ones.
/// Use this only if you don't know the number of strips and vertices ahead of time,
/// otherwise use [`LineBatchesBuilder`] directly!
///
/// Creating new line strip builders is fairly expensive and should be avoided if possible!
pub struct LineDrawableBuilderAllocator<'a> {
    active_line_builder: Option<LineDrawableBuilder>,
    draw_data: Vec<QueueableDrawData>,

    min_num_strips_per_drawable: u32,
    min_num_vertices_per_drawable: u32,

    radius_boost_in_ui_points_for_outlines: f32,

    render_ctx: &'a RenderContext,
}

impl<'a> LineDrawableBuilderAllocator<'a> {
    pub fn new(
        render_ctx: &'a RenderContext,
        min_num_strips_per_drawable: u32,
        min_num_vertices_per_drawable: u32,
        radius_boost_in_ui_points_for_outlines: f32,
    ) -> Self {
        // The internal data texture rows are aligned to 256 bytes.
        // A single line strip takes 8 bytes, making 32 strips the lowest meaningful minimum.
        // A single line vertex takes 16 bytes, making 16 vertices the lowest meaningful minimum.
        const MIN_NUM_STRIPS: u32 = 32;
        const MIN_NUM_VERTICES: u32 = 16;

        Self {
            active_line_builder: None,
            draw_data: Vec::new(),

            // The way we allocate data textures implies that we would waste space if we don't use power of two sizes.
            min_num_strips_per_drawable: min_num_strips_per_drawable
                .max(MIN_NUM_STRIPS)
                .next_power_of_two(),
            min_num_vertices_per_drawable: min_num_vertices_per_drawable
                .max(MIN_NUM_VERTICES)
                .next_power_of_two(),

            radius_boost_in_ui_points_for_outlines,

            render_ctx,
        }
    }

    /// Returns a line strip builder that is guaranteed to have at least the given amount of space.
    pub fn reserve(
        &mut self,
        num_strips: u32,
        num_vertices: u32,
    ) -> Result<&'_ mut LineDrawableBuilder, LineDrawDataError> {
        if let Some(line_builder) = self.active_line_builder.take() {
            self.active_line_builder = if line_builder.remaining_strip_capacity() >= num_strips
                && line_builder.remaining_vertex_capacity() >= num_vertices
            {
                // Finalize previous line builder if out of capacity.
                self.draw_data
                    .push(line_builder.into_draw_data(self.render_ctx)?.into());
                None
            } else {
                Some(line_builder)
            }
        }

        Ok(self.active_line_builder.get_or_insert_with(|| {
            LineDrawableBuilder::new(
                self.render_ctx,
                num_strips.max(self.min_num_strips_per_drawable),
                num_vertices.max(self.min_num_vertices_per_drawable),
            )
            .radius_boost_in_ui_points_for_outlines(self.radius_boost_in_ui_points_for_outlines)
        }))
    }

    pub fn finish(mut self) -> Result<Vec<QueueableDrawData>, LineDrawDataError> {
        if let Some(line_builder) = self.active_line_builder {
            self.draw_data
                .push(line_builder.into_draw_data(self.render_ctx)?.into());
        }
        Ok(self.draw_data)
    }
}
