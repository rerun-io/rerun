use re_log_types::{EntityPath, Instance};
use re_renderer::{
    renderer::{LineDrawDataError, LineStripFlags},
    PickingLayerInstanceId,
};
use re_types::{
    archetypes::GeoLineStrings,
    components::{Color, GeoLineString, Radius},
    Component as _,
};
use re_view::{DataResultQuery as _, RangeResultsExt as _};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewHighlights, ViewQuery, ViewSystemExecutionError,
    VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Default)]
struct GeoLineStringsBatch {
    lines: Vec<Vec<walkers::Position>>,
    radii: Vec<Radius>,
    colors: Vec<re_renderer::Color32>,
    instance_id: Vec<PickingLayerInstanceId>,
}

/// Visualizer for [`GeoLineString`].
#[derive(Default)]
pub struct GeoLineStringsVisualizer {
    batches: Vec<(EntityPath, GeoLineStringsBatch)>,
}

impl IdentifiedViewSystem for GeoLineStringsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "GeoLineStrings".into()
    }
}

impl VisualizerSystem for GeoLineStringsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<GeoLineStrings>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results =
                data_result.query_archetype_with_history::<GeoLineStrings>(ctx, view_query);

            let mut batch_data = GeoLineStringsBatch::default();

            // gather all relevant chunks
            let timeline = view_query.timeline;
            let all_lines = results.iter_as(timeline, GeoLineString::name());
            let all_colors = results.iter_as(timeline, Color::name());
            let all_radii = results.iter_as(timeline, Radius::name());

            // fallback component values
            let fallback_color: Color =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));
            let fallback_radius: Radius =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));

            // iterate over each chunk and find all relevant component slices
            for (_index, lines, colors, radii) in re_query::range_zip_1x2(
                all_lines.primitive_array_list::<2, f64>(),
                all_colors.primitive::<u32>(),
                all_radii.primitive::<f32>(),
            ) {
                // required component
                let lines = lines.as_slice();

                // optional components
                let colors = colors.unwrap_or(&[]);
                let radii = radii.unwrap_or(&[]);

                // optional components values to be used for instance clamping semantics
                let last_color = colors.last().copied().unwrap_or(fallback_color.0 .0);
                let last_radii = radii.last().copied().unwrap_or(fallback_radius.0 .0);

                // iterate over all instances
                for (instance_index, (line, color, radius)) in itertools::izip!(
                    lines,
                    colors.iter().chain(std::iter::repeat(&last_color)),
                    radii.iter().chain(std::iter::repeat(&last_radii)),
                )
                .enumerate()
                {
                    batch_data.lines.push(
                        line.iter()
                            .map(|pos| walkers::Position::from_lat_lon(pos[0], pos[1]))
                            .collect(),
                    );
                    batch_data.radii.push(Radius((*radius).into()));
                    batch_data.colors.push(Color::new(*color).into());
                    batch_data
                        .instance_id
                        .push(re_renderer::PickingLayerInstanceId(instance_index as _));
                }
            }

            self.batches
                .push((data_result.entity_path.clone(), batch_data));
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl GeoLineStringsVisualizer {
    /// Compute the [`super::GeoSpan`] of all the points in the visualizer.
    pub fn span(&self) -> Option<super::GeoSpan> {
        super::GeoSpan::from_lat_long(
            self.batches
                .iter()
                .flat_map(|(_, batch)| batch.lines.iter())
                .flatten()
                .map(|pos| (pos.lat(), pos.lon())),
        )
    }

    pub fn queue_draw_data(
        &self,
        render_ctx: &re_renderer::RenderContext,
        view_builder: &mut re_renderer::ViewBuilder,
        projector: &walkers::Projector,
        highlight: &ViewHighlights,
    ) -> Result<(), LineDrawDataError> {
        let mut lines = re_renderer::LineDrawableBuilder::new(render_ctx);
        lines.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for (entity_path, batch) in &self.batches {
            let outline = highlight.entity_outline_mask(entity_path.hash());

            let mut line_batch = lines
                .batch(entity_path.to_string())
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()))
                .outline_mask_ids(outline.overall);

            let entity_highlight = highlight.entity_outline_mask(entity_path.hash());

            for (strip, radius, color, instance) in itertools::izip!(
                &batch.lines,
                &batch.radii,
                &batch.colors,
                &batch.instance_id
            ) {
                line_batch
                    .add_strip_2d(strip.iter().map(|pos| {
                        let ui_position = projector.project(*pos);
                        glam::vec2(ui_position.x, ui_position.y)
                    }))
                    //TODO(#8013): we use the first vertex's latitude because `re_renderer` doesn't support per-vertex radii
                    .radius(super::radius_to_size(
                        *radius,
                        projector,
                        strip
                            .first()
                            .copied()
                            .unwrap_or(walkers::Position::from_lat_lon(0.0, 0.0)),
                    ))
                    // Looped lines should be connected with rounded corners, so we always add outward extending caps.
                    .flags(LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS)
                    .color(*color)
                    .picking_instance_id(*instance)
                    .outline_mask_ids(
                        entity_highlight.index_outline_mask(Instance::from(instance.0)),
                    );
            }
        }

        view_builder.queue_draw(lines.into_draw_data()?);

        Ok(())
    }
}

impl TypedComponentFallbackProvider<Color> for GeoLineStringsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<Radius> for GeoLineStringsVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        Radius::new_ui_points(2.0)
    }
}

re_viewer_context::impl_component_fallback_provider!(GeoLineStringsVisualizer => [Color, Radius]);
