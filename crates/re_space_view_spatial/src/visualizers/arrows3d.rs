use re_entity_db::{EntityPath, InstancePathHash};
use re_log_types::Instance;
use re_query::range_zip_1x6;
use re_renderer::{renderer::LineStripFlags, LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Arrows3D,
    components::{ClassId, Color, KeypointId, Position3D, Radius, Text, Vector3D},
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
    VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_3d_entities, UiLabel, UiLabelTarget},
};

use super::{
    entity_iterator::clamped, process_annotation_and_keypoint_slices, process_color_slice,
    process_radius_slice, SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Arrows3DVisualizer {
    /// If the number of arrows in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Arrows3DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Arrows3DVisualizer {
    fn process_labels<'a>(
        entity_path: &'a EntityPath,
        vectors: &'a [Vector3D],
        origins: impl Iterator<Item = &'a Position3D> + 'a,
        labels: &'a [Text],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        let labels = clamped(labels, vectors.len());
        let origins = origins.chain(std::iter::repeat(&Position3D::ZERO));
        itertools::izip!(annotation_infos.iter(), vectors, origins, labels, colors)
            .enumerate()
            .filter_map(
                move |(i, (annotation_info, vector, origin, label, color))| {
                    let label = annotation_info.label(Some(label.as_str()));
                    match (vector, label) {
                        (vector, Some(label)) => {
                            let midpoint =
                             // `0.45` rather than `0.5` to account for cap and such
                            (glam::Vec3::from(origin.0) + glam::Vec3::from(vector.0)) * 0.45;
                            let midpoint = world_from_obj.transform_point3(midpoint);

                            Some(UiLabel {
                                text: label,
                                color: *color,
                                target: UiLabelTarget::Position3D(
                                    world_from_obj.transform_point3(midpoint),
                                ),
                                labeled_instance: InstancePathHash::instance(
                                    entity_path,
                                    Instance::from(i as u64),
                                ),
                            })
                        }
                        _ => None,
                    }
                },
            )
    }

    fn process_data<'a>(
        &mut self,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Arrows3DComponentData<'a>>,
    ) {
        for data in data {
            let num_instances = data.vectors.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.vectors.iter().map(|_| glam::Vec3::ZERO),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            let radii = process_radius_slice(entity_path, num_instances, data.radii);
            let colors =
                process_color_slice(entity_path, num_instances, &annotation_infos, data.colors);

            if num_instances <= self.max_labels {
                let origins = clamped(data.origins, num_instances);
                self.data.ui_labels.extend(Self::process_labels(
                    entity_path,
                    data.vectors,
                    origins,
                    data.labels,
                    &colors,
                    &annotation_infos,
                    ent_context.world_from_entity,
                ));
            }

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut bounding_box = macaw::BoundingBox::nothing();

            let origins =
                clamped(data.origins, num_instances).chain(std::iter::repeat(&Position3D::ZERO));
            for (i, (vector, origin, radius, color)) in
                itertools::izip!(data.vectors, origins, radii, colors).enumerate()
            {
                let vector: glam::Vec3 = vector.0.into();
                let origin: glam::Vec3 = origin.0.into();
                let end = origin + vector;

                let segment = line_batch
                    .add_segment(origin, end)
                    .radius(radius)
                    .color(color)
                    .flags(
                        LineStripFlags::FLAG_COLOR_GRADIENT
                            | LineStripFlags::FLAG_CAP_END_TRIANGLE
                            | LineStripFlags::FLAG_CAP_START_ROUND
                            | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS,
                    )
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    segment.outline_mask_ids(*outline_mask_ids);
                }

                bounding_box.extend(origin);
                bounding_box.extend(end);
            }

            self.data.add_bounding_box(
                entity_path.hash(),
                bounding_box,
                ent_context.world_from_entity,
            );
        }
    }
}

// ---

struct Arrows3DComponentData<'a> {
    // Point of views
    vectors: &'a [Vector3D],

    // Clamped to edge
    origins: &'a [Position3D],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for Arrows3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Arrows3D".into()
    }
}

impl VisualizerSystem for Arrows3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Arrows3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype::<Self, Arrows3D, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, entity_path, _entity_props, spatial_ctx, results| {
                re_tracing::profile_scope!(format!("{entity_path}"));

                use crate::visualizers::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let vectors = match results.get_dense::<Vector3D>(resolver) {
                    Some(vectors) => vectors?,
                    _ => return Ok(()),
                };

                let num_vectors = vectors
                    .range_indexed()
                    .map(|(_, vectors)| vectors.len())
                    .sum::<usize>();
                if num_vectors == 0 {
                    return Ok(());
                }

                line_builder.reserve_strips(num_vectors)?;
                line_builder.reserve_vertices(num_vectors * 2)?;

                let origins = results.get_or_empty_dense(resolver)?;
                let colors = results.get_or_empty_dense(resolver)?;
                let radii = results.get_or_empty_dense(resolver)?;
                let labels = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;
                let keypoint_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x6(
                    vectors.range_indexed(),
                    origins.range_indexed(),
                    colors.range_indexed(),
                    radii.range_indexed(),
                    labels.range_indexed(),
                    class_ids.range_indexed(),
                    keypoint_ids.range_indexed(),
                )
                .map(
                    |(_index, vectors, origins, colors, radii, labels, class_ids, keypoint_ids)| {
                        Arrows3DComponentData {
                            vectors,
                            origins: origins.unwrap_or_default(),
                            colors: colors.unwrap_or_default(),
                            radii: radii.unwrap_or_default(),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids.unwrap_or_default(),
                            keypoint_ids: keypoint_ids.unwrap_or_default(),
                        }
                    },
                );

                self.process_data(
                    &mut line_builder,
                    view_query,
                    entity_path,
                    spatial_ctx,
                    data,
                );

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
}
