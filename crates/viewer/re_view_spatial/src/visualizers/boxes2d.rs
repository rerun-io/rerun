use re_log_types::Instance;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Boxes2D,
    components::{ClassId, Color, DrawOrder, HalfSize2D, Position2D, Radius, ShowLabels, Text},
    ArrowString, Component as _,
};
use re_view::{process_annotation_slices, process_color_slice};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialViewKind,
    visualizers::{entity_iterator::clamped_or, UiLabelTarget},
};

use super::{
    filter_visualizable_2d_entities, process_radius_slice,
    utilities::{process_labels, LabeledBatch},
    SpatialViewVisualizerData,
};

// ---

pub struct Boxes2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Boxes2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Boxes2DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        view_query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Boxes2DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.half_sizes.len();
            if num_instances == 0 {
                continue;
            }

            let annotation_infos = process_annotation_slices(
                view_query.latest_at,
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
                .single_entity_transform_required(entity_path, "Boxes2D");

            let mut line_batch = line_builder
                .batch("boxes2d")
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = re_math::BoundingBox::NOTHING;

            let centers = clamped_or(data.centers, &Position2D::ZERO);

            for (i, (half_size, center, radius, &color)) in
                itertools::izip!(data.half_sizes, centers, radii, &colors).enumerate()
            {
                let min = half_size.box_min(*center);
                let max = half_size.box_max(*center);
                obj_space_bounding_box.extend(min.extend(0.0));
                obj_space_bounding_box.extend(max.extend(0.0));

                let rectangle = line_batch
                    .add_rectangle_outline_2d(
                        min,
                        glam::vec2(half_size.width(), 0.0),
                        glam::vec2(0.0, half_size.height()),
                    )
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(PickingLayerInstanceId(i as _));
                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    rectangle.outline_mask_ids(*outline_mask_ids);
                }
            }

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            self.data.ui_labels.extend(process_labels(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: UiLabelTarget::Point2D(
                        <[f32; 2]>::from(obj_space_bounding_box.center().truncate()).into(),
                    ),
                    instance_positions: data
                        .half_sizes
                        .iter()
                        .copied()
                        .zip(clamped_or(data.centers, &Position2D::ZERO).copied())
                        .map(|(half_size, center)| {
                            let min = half_size.box_min(center);
                            let max = half_size.box_max(center);
                            UiLabelTarget::Rect(egui::Rect::from_min_max(
                                egui::pos2(min.x, min.y),
                                egui::pos2(max.x, max.y),
                            ))
                        }),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| self.fallback_for(ctx)),
                    annotation_infos: &annotation_infos,
                },
                std::convert::identity,
            ));
        }
    }
}

// ---

struct Boxes2DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize2D],

    // Clamped to edge
    centers: &'a [Position2D],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Boxes2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes2D".into()
    }
}

impl VisualizerSystem for Boxes2DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes2D>()
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(ViewSystemExecutionError::NoRenderContextError);
        };

        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        use super::entity_iterator::{iter_primitive_array, process_archetype};
        process_archetype::<Self, Boxes2D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_half_size_chunks) = results.get_required_chunks(&HalfSize2D::name())
                else {
                    return Ok(());
                };

                let num_boxes: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive_array::<2, f32>(&HalfSize2D::name()))
                    .map(|vectors| vectors.len())
                    .sum();
                if num_boxes == 0 {
                    return Ok(());
                }

                // Each box consists of one strip with a total of 5 vertices each.
                line_builder.reserve_strips(num_boxes)?;
                line_builder.reserve_vertices(num_boxes * 5)?;

                let timeline = ctx.query.timeline();
                let all_half_sizes_indexed = iter_primitive_array::<2, f32>(
                    &all_half_size_chunks,
                    timeline,
                    HalfSize2D::name(),
                );
                let all_centers = results.iter_as(timeline, Position2D::name());
                let all_colors = results.iter_as(timeline, Color::name());
                let all_radii = results.iter_as(timeline, Radius::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());
                let all_show_labels = results.iter_as(timeline, ShowLabels::name());

                let data = re_query::range_zip_1x6(
                    all_half_sizes_indexed,
                    all_centers.primitive_array::<2, f32>(),
                    all_colors.primitive::<u32>(),
                    all_radii.primitive::<f32>(),
                    all_labels.string(),
                    all_class_ids.primitive::<u16>(),
                    all_show_labels.bool(),
                )
                .map(
                    |(
                        _index,
                        half_sizes,
                        centers,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        show_labels,
                    )| {
                        Boxes2DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            centers: centers.map_or(&[], |centers| bytemuck::cast_slice(centers)),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels.unwrap_or_default().get(0).map(Into::into),
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

impl TypedComponentFallbackProvider<Color> for Boxes2DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for Boxes2DVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_BOX2D
    }
}

impl TypedComponentFallbackProvider<ShowLabels> for Boxes2DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ShowLabels {
        super::utilities::show_labels_fallback::<HalfSize2D>(ctx)
    }
}

re_viewer_context::impl_component_fallback_provider!(Boxes2DVisualizer => [Color, DrawOrder, ShowLabels]);
