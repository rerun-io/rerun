use re_log_types::Instance;
use re_renderer::{PickingLayerInstanceId, renderer::LineStripFlags};
use re_types::{
    Archetype as _, ArrowString,
    archetypes::LineStrips3D,
    components::{ClassId, Color, Radius, ShowLabels},
};
use re_view::{process_annotation_slices, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem, auto_color_for_entity_path,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialViewKind,
    visualizers::utilities::{LabeledBatch, process_labels_3d},
};

use super::{SpatialViewVisualizerData, filter_visualizable_3d_entities, process_radius_slice};

// ---

pub struct Lines3DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines3DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::ThreeD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Lines3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Lines3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.strips.len();
            if num_instances == 0 {
                continue;
            }

            let annotation_infos = process_annotation_slices(
                query.latest_at,
                num_instances,
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
                .single_entity_transform_required(entity_path, LineStrips3D::name());

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = macaw::BoundingBox::nothing();

            let mut num_rendered_strips = 0usize;
            for (i, (strip, radius, &color)) in
                itertools::izip!(data.strips.iter(), radii, &colors).enumerate()
            {
                let lines = line_batch
                    .add_strip(strip.iter().copied().map(Into::into))
                    // Looped lines should be connected with rounded corners, so we always add outward extending caps.
                    .flags(LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS)
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

                for p in *strip {
                    obj_space_bounding_box.extend((*p).into());
                }

                num_rendered_strips += 1;
            }
            debug_assert_eq!(
                data.strips.len(),
                num_rendered_strips,
                "the number of renderer strips after all post-processing is done should be equal to {} (got {num_rendered_strips} instead)",
                data.strips.len()
            );

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            self.data.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: obj_space_bounding_box.center(),
                    instance_positions: data.strips.iter().map(|strip| {
                        strip
                            .iter()
                            .copied()
                            .map(glam::Vec3::from)
                            .sum::<glam::Vec3>()
                            / (strip.len() as f32)
                    }),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| self.fallback_for(ctx)),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }
    }
}

// ---

struct Lines3DComponentData<'a> {
    // Point of views
    strips: Vec<&'a [[f32; 3]]>,

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Lines3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Lines3D".into()
    }
}

impl VisualizerSystem for Lines3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<LineStrips3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: MaybeVisualizableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, LineStrips3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_strip_chunks) =
                    results.get_required_chunks(LineStrips3D::descriptor_strips())
                else {
                    return Ok(());
                };

                let num_strips = all_strip_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<&[[f32; 3]]>())
                    .map(|strips| strips.len())
                    .sum();
                if num_strips == 0 {
                    return Ok(());
                }
                line_builder.reserve_strips(num_strips)?;

                let num_vertices = all_strip_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<&[[f32; 3]]>())
                    .map(|strips| strips.iter().map(|strip| strip.len()).sum::<usize>())
                    .sum::<usize>();
                line_builder.reserve_vertices(num_vertices)?;

                let timeline = ctx.query.timeline();
                let all_strips_indexed = iter_slices::<&[[f32; 3]]>(&all_strip_chunks, timeline);
                let all_colors = results.iter_as(timeline, LineStrips3D::descriptor_colors());
                let all_radii = results.iter_as(timeline, LineStrips3D::descriptor_radii());
                let all_labels = results.iter_as(timeline, LineStrips3D::descriptor_labels());
                let all_class_ids = results.iter_as(timeline, LineStrips3D::descriptor_class_ids());
                let all_show_labels =
                    results.iter_as(timeline, LineStrips3D::descriptor_show_labels());

                let data = re_query::range_zip_1x5(
                    all_strips_indexed,
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(_index, strips, colors, radii, labels, class_ids, show_labels)| {
                        Lines3DComponentData {
                            strips,
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
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

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for Lines3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<ShowLabels> for Lines3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ShowLabels {
        super::utilities::show_labels_fallback(
            ctx,
            &LineStrips3D::descriptor_strips(),
            &LineStrips3D::descriptor_labels(),
        )
    }
}

re_viewer_context::impl_component_fallback_provider!(Lines3DVisualizer => [Color, ShowLabels]);
