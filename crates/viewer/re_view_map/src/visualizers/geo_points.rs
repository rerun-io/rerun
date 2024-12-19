use re_log_types::EntityPath;
use re_renderer::{renderer::PointCloudDrawDataError, PickingLayerInstanceId};
use re_types::{
    archetypes::GeoPoints,
    components::{ClassId, Color, LatLon, Radius},
    Component as _,
};
use re_view::{
    process_annotation_slices, process_color_slice, AnnotationSceneContext, DataResultQuery as _,
    RangeResultsExt as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewHighlights, ViewQuery, ViewSystemExecutionError,
    VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Default)]
pub struct GeoPointBatch {
    pub positions: Vec<walkers::Position>,
    pub radii: Vec<Radius>,
    pub colors: Vec<re_renderer::Color32>,
    pub instance_id: Vec<PickingLayerInstanceId>,
}

/// Visualizer for [`GeoPoints`].
#[derive(Default)]
pub struct GeoPointsVisualizer {
    batches: Vec<(EntityPath, GeoPointBatch)>,
}

impl IdentifiedViewSystem for GeoPointsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "GeoPoints".into()
    }
}

impl VisualizerSystem for GeoPointsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<GeoPoints>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let annotation_scene_context = context_systems.get::<AnnotationSceneContext>()?;
        let latest_at_query = view_query.latest_at_query();

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result.query_archetype_with_history::<GeoPoints>(ctx, view_query);
            let annotation_context = annotation_scene_context.0.find(&data_result.entity_path);

            let mut batch_data = GeoPointBatch::default();

            // gather all relevant chunks
            let timeline = view_query.timeline;
            let all_positions = results.iter_as(timeline, LatLon::name());
            let all_colors = results.iter_as(timeline, Color::name());
            let all_radii = results.iter_as(timeline, Radius::name());
            let all_class_ids = results.iter_as(timeline, ClassId::name());

            // fallback component values
            let query_context = ctx.query_context(data_result, &latest_at_query);
            let fallback_radius: Radius =
                self.fallback_for(&ctx.query_context(data_result, &latest_at_query));

            // iterate over each chunk and find all relevant component slices
            for (_index, positions, colors, radii, class_ids) in re_query::range_zip_1x3(
                all_positions.primitive_array::<2, f64>(),
                all_colors.primitive::<u32>(),
                all_radii.primitive::<f32>(),
                all_class_ids.primitive::<u16>(),
            ) {
                // required component
                let num_instances = positions.len();

                // Resolve annotation info (if needed).
                let annotation_infos = process_annotation_slices(
                    view_query.latest_at,
                    num_instances,
                    class_ids.map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                    &annotation_context,
                );

                // optional components
                let colors = process_color_slice(
                    &query_context,
                    self,
                    num_instances,
                    &annotation_infos,
                    colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                );
                let radii = radii.unwrap_or(&[]);

                // optional components values to be used for instance clamping semantics
                let last_radii = radii.last().copied().unwrap_or(fallback_radius.0 .0);

                // iterate over all instances
                for (instance_index, (position, color, radius)) in itertools::izip!(
                    positions,
                    colors.iter(),
                    radii.iter().chain(std::iter::repeat(&last_radii)),
                )
                .enumerate()
                {
                    batch_data
                        .positions
                        .push(walkers::Position::from_lat_lon(position[0], position[1]));
                    batch_data.radii.push(Radius((*radius).into()));
                    batch_data.colors.push(*color);
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

impl GeoPointsVisualizer {
    /// Compute the [`super::GeoSpan`] of all the points in the visualizer.
    pub fn span(&self) -> Option<super::GeoSpan> {
        super::GeoSpan::from_lat_long(
            self.batches
                .iter()
                .flat_map(|(_, batch)| batch.positions.iter())
                .map(|pos| (pos.lat(), pos.lon())),
        )
    }

    pub fn queue_draw_data(
        &self,
        render_ctx: &re_renderer::RenderContext,
        view_builder: &mut re_renderer::ViewBuilder,
        projector: &walkers::Projector,
        highlight: &ViewHighlights,
    ) -> Result<(), PointCloudDrawDataError> {
        let mut points = re_renderer::PointCloudBuilder::new(render_ctx);
        // NOTE: Do not `points.radius_boost_in_ui_points_for_outlines`! The points are not shaded,
        // so boosting the outline radius would make it erreously large.

        for (entity_path, batch) in &self.batches {
            let (positions, radii): (Vec<_>, Vec<_>) = batch
                .positions
                .iter()
                .zip(&batch.radii)
                .map(|(pos, radius)| {
                    let size = super::radius_to_size(*radius, projector, *pos);
                    let ui_position = projector.project(*pos);
                    (glam::vec3(ui_position.x, ui_position.y, 0.0), size)
                })
                .unzip();

            let outline = highlight.entity_outline_mask(entity_path.hash());

            let mut point_batch = points
                .batch_with_info(re_renderer::renderer::PointCloudBatchInfo {
                    label: entity_path.to_string().into(),
                    flags: re_renderer::renderer::PointCloudBatchFlags::empty(),
                    ..re_renderer::renderer::PointCloudBatchInfo::default()
                })
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()))
                .outline_mask_ids(outline.overall);

            //TODO(ab, andreas): boilerplate copy-pasted from points2d
            let num_instances = positions.len() as u64;
            for (highlighted_key, instance_mask_ids) in &outline.instances {
                let highlighted_point_index =
                    (highlighted_key.get() < num_instances).then_some(highlighted_key.get());
                if let Some(highlighted_point_index) = highlighted_point_index {
                    point_batch = point_batch.push_additional_outline_mask_ids_for_range(
                        highlighted_point_index as u32..highlighted_point_index as u32 + 1,
                        *instance_mask_ids,
                    );
                }
            }

            point_batch.add_points_2d(&positions, &radii, &batch.colors, &batch.instance_id);
        }

        view_builder.queue_draw(points.into_draw_data()?);

        Ok(())
    }
}

impl TypedComponentFallbackProvider<Color> for GeoPointsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<Radius> for GeoPointsVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        Radius::new_ui_points(5.0)
    }
}

re_viewer_context::impl_component_fallback_provider!(GeoPointsVisualizer => [Color, Radius]);
