use re_log_types::Instance;
use re_query::range_zip_1x5;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::LineStrips2D,
    components::{ClassId, Color, DrawOrder, KeypointId, LineStrip2D, Radius, Text},
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{contexts::SpatialSceneEntityContext, view_kind::SpatialSpaceViewKind};

use super::{
    filter_visualizable_2d_entities, process_annotation_and_keypoint_slices, process_color_slice,
    process_labels_2d, process_radius_slice, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Lines2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Lines2DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Lines2DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.strips.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.strips.iter().map(|_| glam::Vec3::ZERO),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors =
                process_color_slice(ctx, self, num_instances, &annotation_infos, data.colors);

            let world_from_obj = ent_context
                .transform_info
                .single_entity_transform_required(entity_path, "Lines2D");

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = re_math::BoundingBox::NOTHING;
            for (i, (strip, radius, &color)) in
                itertools::izip!(data.strips, radii, &colors).enumerate()
            {
                let lines = line_batch
                    .add_strip_2d(strip.0.iter().copied().map(Into::into))
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    lines.outline_mask_ids(*outline_mask_ids);
                }

                for p in &strip.0 {
                    obj_space_bounding_box.extend(glam::vec3(p.x(), p.y(), 0.0));
                }
            }

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            if data.labels.len() == 1 || num_instances <= super::MAX_NUM_LABELS_PER_ENTITY {
                // If there's many strips but only a single label, place the single label at the middle of the visualization.
                let label_positions = if data.labels.len() == 1 && data.strips.len() > 1 {
                    // TODO(andreas): A smoothed over time (+ discontinuity detection) bounding box would be great.
                    itertools::Either::Left(std::iter::once(
                        obj_space_bounding_box.center().truncate(),
                    ))
                } else {
                    // Take middle point of every strip.
                    itertools::Either::Right(data.strips.iter().map(|strip| {
                        strip
                            .0
                            .iter()
                            .copied()
                            .map(glam::Vec2::from)
                            .sum::<glam::Vec2>()
                            / (strip.0.len() as f32)
                    }))
                };

                self.data.ui_labels.extend(process_labels_2d(
                    entity_path,
                    label_positions,
                    data.labels,
                    &colors,
                    &annotation_infos,
                    world_from_obj,
                ));
            }
        }
    }
}

// ---

struct Lines2DComponentData<'a> {
    // Point of views
    strips: &'a [LineStrip2D],

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for Lines2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Lines2D".into()
    }
}

impl VisualizerSystem for Lines2DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<LineStrips2D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_2d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype::<Self, LineStrips2D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let strips = match results.get_required_component_dense::<LineStrip2D>(resolver) {
                    Some(strips) => strips?,
                    _ => return Ok(()),
                };

                let num_strips = strips
                    .range_indexed()
                    .map(|(_, strips)| strips.len())
                    .sum::<usize>();
                if num_strips == 0 {
                    return Ok(());
                }
                line_builder.reserve_strips(num_strips)?;

                let num_vertices = strips
                    .range_indexed()
                    .map(|(_, strips)| strips.iter().map(|strip| strip.0.len()).sum::<usize>())
                    .sum::<usize>();
                line_builder.reserve_vertices(num_vertices)?;

                let colors = results.get_or_empty_dense(resolver)?;
                let radii = results.get_or_empty_dense(resolver)?;
                let labels = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;
                let keypoint_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x5(
                    strips.range_indexed(),
                    colors.range_indexed(),
                    radii.range_indexed(),
                    labels.range_indexed(),
                    class_ids.range_indexed(),
                    keypoint_ids.range_indexed(),
                )
                .map(
                    |(_index, strips, colors, radii, labels, class_ids, keypoint_ids)| {
                        Lines2DComponentData {
                            strips,
                            colors: colors.unwrap_or_default(),
                            radii: radii.unwrap_or_default(),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids.unwrap_or_default(),
                            keypoint_ids: keypoint_ids.unwrap_or_default(),
                        }
                    },
                );

                self.process_data(ctx, &mut line_builder, view_query, spatial_ctx, data);

                Ok(())
            },
        )?;

        Ok(vec![(line_builder.into_draw_data()?.into())])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for Lines2DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for Lines2DVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_LINES2D
    }
}

re_viewer_context::impl_component_fallback_provider!(Lines2DVisualizer => [Color, DrawOrder]);
