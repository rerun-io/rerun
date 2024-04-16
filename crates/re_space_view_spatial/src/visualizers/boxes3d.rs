use re_entity_db::{EntityPath, InstancePathHash};
use re_query_cache::{range_zip_1x7, CachedResults};
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Boxes3D,
    components::{
        ClassId, Color, HalfSizes3D, InstanceKey, KeypointId, Position3D, Radius, Rotation3D, Text,
    },
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
    VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{UiLabel, UiLabelTarget},
};

use super::{
    entity_iterator::clamped, filter_visualizable_3d_entities,
    process_annotation_and_keypoint_slices, process_color_slice, process_radius_slice,
    SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Boxes3DVisualizer {
    fn process_labels<'a>(
        entity_path: &'a EntityPath,
        half_sizes: &'a [HalfSizes3D],
        centers: impl Iterator<Item = &'a Position3D> + 'a,
        labels: &'a [Text],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_entity: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        let labels = clamped(labels, half_sizes.len());
        let centers = centers.chain(std::iter::repeat(&Position3D::ZERO));
        itertools::izip!(annotation_infos.iter(), centers, labels, colors)
            .enumerate()
            .filter_map(move |(i, (annotation_info, center, label, color))| {
                let label = annotation_info.label(Some(label.as_str()));
                label.map(|label| UiLabel {
                    text: label,
                    color: *color,
                    target: UiLabelTarget::Position3D(
                        world_from_entity.transform_point3(center.0.into()),
                    ),
                    labeled_instance: InstancePathHash::instance(entity_path, InstanceKey(i as _)),
                })
            })
    }

    fn process_data<'a>(
        &mut self,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Boxes3DComponentData<'a>>,
    ) {
        for data in data {
            let num_instances = data.half_sizes.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.half_sizes.iter().map(|_| glam::Vec3::ZERO),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            let radii = process_radius_slice(entity_path, num_instances, data.radii);
            let colors =
                process_color_slice(entity_path, num_instances, &annotation_infos, data.colors);

            let centers = clamped(data.centers, num_instances);
            self.0.ui_labels.extend(Self::process_labels(
                entity_path,
                data.half_sizes,
                centers,
                data.labels,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            ));

            let mut line_batch = line_builder
                .batch("boxes3d")
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut bounding_box = macaw::BoundingBox::nothing();

            let centers =
                clamped(data.centers, num_instances).chain(std::iter::repeat(&Position3D::ZERO));
            let rotations = clamped(data.rotations, num_instances)
                .chain(std::iter::repeat(&Rotation3D::IDENTITY));
            for (i, (half_size, &center, rotation, radius, color)) in
                itertools::izip!(data.half_sizes, centers, rotations, radii, colors).enumerate()
            {
                bounding_box.extend(half_size.box_min(center));
                bounding_box.extend(half_size.box_max(center));

                let center = center.into();

                let box3d = line_batch
                    .add_box_outline_from_transform(
                        glam::Affine3A::from_scale_rotation_translation(
                            glam::Vec3::from(*half_size) * 2.0,
                            rotation.0.into(),
                            center,
                        ),
                    )
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) =
                    ent_context.highlight.instances.get(&InstanceKey(i as _))
                {
                    box3d.outline_mask_ids(*outline_mask_ids);
                }
            }

            self.0.add_bounding_box(
                entity_path.hash(),
                bounding_box,
                ent_context.world_from_entity,
            );
        }
    }
}

// ---

struct Boxes3DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSizes3D],

    // Clamped to edge
    centers: &'a [Position3D],
    rotations: &'a [Rotation3D],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes3D>()
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

        super::entity_iterator::process_archetype::<Boxes3DVisualizer, Boxes3D, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, entity_path, _entity_props, spatial_ctx, results| {
                match results {
                    CachedResults::LatestAt(_query, results) => {
                        re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                        use crate::visualizers::CachedLatestAtResultsExt as _;

                        let resolver = ctx.recording().resolver();

                        let half_sizes = match results.get_dense::<HalfSizes3D>(resolver) {
                            Some(Ok(vectors)) if !vectors.is_empty() => vectors,
                            Some(err @ Err(_)) => err?,
                            _ => return Ok(()),
                        };

                        // Each box consists of 12 independent lines with 2 vertices each.
                        line_builder.reserve_strips(half_sizes.len() * 12)?;
                        line_builder.reserve_vertices(half_sizes.len() * 12 * 2)?;

                        let centers = results.get_or_empty_dense(resolver)?;
                        let rotations = results.get_or_empty_dense(resolver)?;
                        let colors = results.get_or_empty_dense(resolver)?;
                        let radii = results.get_or_empty_dense(resolver)?;
                        let labels = results.get_or_empty_dense(resolver)?;
                        let class_ids = results.get_or_empty_dense(resolver)?;
                        let keypoint_ids = results.get_or_empty_dense(resolver)?;

                        let data = Boxes3DComponentData {
                            half_sizes,
                            centers,
                            rotations,
                            colors,
                            radii,
                            labels,
                            keypoint_ids,
                            class_ids,
                        };

                        self.process_data(
                            &mut line_builder,
                            view_query,
                            entity_path,
                            spatial_ctx,
                            std::iter::once(data),
                        );
                    }

                    CachedResults::Range(_query, results) => {
                        re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                        use crate::visualizers::CachedRangeResultsExt as _;

                        let resolver = ctx.recording().resolver();

                        let half_sizes = match results.get_dense::<HalfSizes3D>(resolver, _query) {
                            Some(Ok(vectors)) => vectors,
                            Some(err @ Err(_)) => err?,
                            _ => return Ok(()),
                        };

                        let num_boxes = half_sizes
                            .range_indexed(_query.range())
                            .map(|(_, vectors)| vectors.len())
                            .sum::<usize>();
                        if num_boxes == 0 {
                            return Ok(());
                        }

                        // Each box consists of 12 independent lines with 2 vertices each.
                        line_builder.reserve_strips(num_boxes * 12)?;
                        line_builder.reserve_vertices(num_boxes * 12 * 2)?;

                        let centers = results.get_or_empty_dense(resolver, _query)?;
                        let rotations = results.get_or_empty_dense(resolver, _query)?;
                        let colors = results.get_or_empty_dense(resolver, _query)?;
                        let radii = results.get_or_empty_dense(resolver, _query)?;
                        let labels = results.get_or_empty_dense(resolver, _query)?;
                        let class_ids = results.get_or_empty_dense(resolver, _query)?;
                        let keypoint_ids = results.get_or_empty_dense(resolver, _query)?;

                        let data = range_zip_1x7(
                            half_sizes.range_indexed(_query.range()),
                            centers.range_indexed(_query.range()),
                            rotations.range_indexed(_query.range()),
                            colors.range_indexed(_query.range()),
                            radii.range_indexed(_query.range()),
                            labels.range_indexed(_query.range()),
                            class_ids.range_indexed(_query.range()),
                            keypoint_ids.range_indexed(_query.range()),
                        )
                        .map(
                            |(
                                _index,
                                half_sizes,
                                centers,
                                rotations,
                                colors,
                                radii,
                                labels,
                                class_ids,
                                keypoint_ids,
                            )| {
                                Boxes3DComponentData {
                                    half_sizes,
                                    centers: centers.unwrap_or_default(),
                                    rotations: rotations.unwrap_or_default(),
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
                    }
                }

                Ok(())
            },
        )?;

        Ok(vec![(line_builder.into_draw_data()?.into())])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
