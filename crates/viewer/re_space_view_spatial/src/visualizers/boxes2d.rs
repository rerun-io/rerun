use re_entity_db::{EntityPath, InstancePathHash};
use re_log_types::Instance;
use re_query::range_zip_1x6;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Boxes2D,
    components::{ClassId, Color, DrawOrder, HalfSize2D, KeypointId, Position2D, Radius, Text},
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    ResolvedAnnotationInfos, SpaceViewSystemExecutionError, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialSpaceViewKind,
    visualizers::{entity_iterator::clamped_or, UiLabel, UiLabelTarget},
};

use super::{
    filter_visualizable_2d_entities, process_annotation_and_keypoint_slices, process_color_slice,
    process_labels_2d, process_radius_slice, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Boxes2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Boxes2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Boxes2DVisualizer {
    /// Produces 2D rect ui labels from component data.
    ///
    /// Does nothing if there's no positions or no labels passed.
    /// Assumes that there's at least a single color in `colors`.
    /// Otherwise, produces one label per center position passed.
    fn process_labels<'a>(
        entity_path: &'a EntityPath,
        half_sizes: &'a [HalfSize2D],
        centers: impl Iterator<Item = &'a Position2D> + 'a,
        labels: &'a [Text],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        debug_assert!(
            labels.is_empty() || !colors.is_empty(),
            "Cannot add labels without colors"
        );

        let labels = annotation_infos
            .iter()
            .zip(labels.iter().map(Some).chain(std::iter::repeat(None)))
            .map(|(annotation_info, label)| annotation_info.label(label.map(|l| l.as_str())));

        let colors = clamped_or(colors, &egui::Color32::WHITE);

        itertools::izip!(half_sizes, centers, labels, colors)
            .enumerate()
            .filter_map(move |(i, (half_size, center, label, color))| {
                label.map(|label| {
                    let min = half_size.box_min(*center);
                    let max = half_size.box_max(*center);
                    UiLabel {
                        text: label,
                        color: *color,
                        target: UiLabelTarget::Rect(egui::Rect::from_min_max(
                            egui::pos2(min.x, min.y),
                            egui::pos2(max.x, max.y),
                        )),
                        labeled_instance: InstancePathHash::instance(
                            entity_path,
                            Instance::from(i as u64),
                        ),
                    }
                })
            })
    }

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

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                view_query.latest_at,
                num_instances,
                data.half_sizes.iter().map(|_| glam::Vec3::ZERO),
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

            let mut line_batch = line_builder
                .batch("boxes2d")
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(ent_context.world_from_entity)
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

            self.data.add_bounding_box(
                entity_path.hash(),
                obj_space_bounding_box,
                ent_context.world_from_entity,
            );

            if data.labels.len() == 1 || num_instances <= super::MAX_NUM_LABELS_PER_ENTITY {
                if data.labels.len() == 1 && num_instances > 1 {
                    // If there's many boxes but only a single label, place the single label at the middle of the visualization.
                    // TODO(andreas): A smoothed over time (+ discontinuity detection) bounding box would be great.
                    self.data.ui_labels.extend(process_labels_2d(
                        entity_path,
                        std::iter::once(obj_space_bounding_box.center().truncate()),
                        data.labels,
                        &colors,
                        &annotation_infos,
                        ent_context.world_from_entity,
                    ));
                } else {
                    let centers = clamped_or(data.centers, &Position2D::ZERO);
                    self.data.ui_labels.extend(Self::process_labels(
                        entity_path,
                        data.half_sizes,
                        centers,
                        data.labels,
                        &colors,
                        &annotation_infos,
                    ));
                }
            }
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
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype::<Self, Boxes2D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let half_sizes = match results.get_required_component_dense::<HalfSize2D>(resolver)
                {
                    Some(vectors) => vectors?,
                    _ => return Ok(()),
                };

                let num_boxes = half_sizes
                    .range_indexed()
                    .map(|(_, vectors)| vectors.len())
                    .sum::<usize>();
                if num_boxes == 0 {
                    return Ok(());
                }

                // Each box consists of 4 independent lines of 2 vertices each.
                line_builder.reserve_strips(num_boxes * 4)?;
                line_builder.reserve_vertices(num_boxes * 4 * 2)?;

                let centers = results.get_or_empty_dense(resolver)?;
                let colors = results.get_or_empty_dense(resolver)?;
                let radii = results.get_or_empty_dense(resolver)?;
                let labels = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;
                let keypoint_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x6(
                    half_sizes.range_indexed(),
                    centers.range_indexed(),
                    colors.range_indexed(),
                    radii.range_indexed(),
                    labels.range_indexed(),
                    class_ids.range_indexed(),
                    keypoint_ids.range_indexed(),
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
                        keypoint_ids,
                    )| {
                        Boxes2DComponentData {
                            half_sizes,
                            centers: centers.unwrap_or_default(),
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

re_viewer_context::impl_component_fallback_provider!(Boxes2DVisualizer => [Color, DrawOrder]);
