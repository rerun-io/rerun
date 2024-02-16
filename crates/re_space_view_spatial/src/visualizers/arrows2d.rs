use re_entity_db::{EntityPath, InstancePathHash};
use re_renderer::{renderer::LineStripFlags, LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Arrows2D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position2D, Radius, Text, Vector2D},
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
    VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use super::{
    process_annotation_and_keypoint_slices, process_color_slice, process_radius_slice,
    SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_2d_entities, UiLabel, UiLabelTarget},
};

pub struct Arrows2DVisualizer {
    /// If the number of arrows in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Arrows2DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Arrows2DVisualizer {
    fn process_labels<'a>(
        vectors: &'a [Vector2D],
        origins: impl Iterator<Item = Option<Position2D>> + 'a,
        labels: &'a [Option<Text>],
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        itertools::izip!(
            annotation_infos.iter(),
            vectors,
            origins,
            labels,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, vector, origin, label, color, labeled_instance)| {
                let origin = origin.unwrap_or(Position2D::ZERO);
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                match (vector, label) {
                    (vector, Some(label)) => {
                        let midpoint =
                             // `0.45` rather than `0.5` to account for cap and such
                            glam::Vec2::from(origin.0) + glam::Vec2::from(vector.0) * 0.45;
                        let midpoint = world_from_obj.transform_point3(midpoint.extend(0.0));

                        Some(UiLabel {
                            text: label,
                            color: *color,
                            target: UiLabelTarget::Point2D(egui::pos2(midpoint.x, midpoint.y)),
                            labeled_instance: *labeled_instance,
                        })
                    }
                    _ => None,
                }
            },
        )
    }

    fn process_data(
        &mut self,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        data: &Arrows2DComponentData<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) {
        let (annotation_infos, _) = process_annotation_and_keypoint_slices(
            query.latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.vectors.iter().map(|_| glam::Vec3::ZERO),
            &ent_context.annotations,
        );

        let radii = process_radius_slice(data.radii, data.vectors.len(), ent_path);
        let colors = process_color_slice(data.colors, ent_path, &annotation_infos);
        let origins = || {
            data.origins.map_or_else(
                || itertools::Either::Left(std::iter::repeat(Some(Position2D::ZERO))),
                |origins| itertools::Either::Right(origins.iter().copied()),
            )
        };

        if data.instance_keys.len() <= self.max_labels {
            re_tracing::profile_scope!("labels");

            // Max labels is small enough that we can afford iterating on the colors again.
            let colors = process_color_slice(data.colors, ent_path, &annotation_infos);

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                data.instance_keys
                    .iter()
                    .copied()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            if let Some(labels) = data.labels {
                self.data.ui_labels.extend(Self::process_labels(
                    data.vectors,
                    origins(),
                    labels,
                    &instance_path_hashes_for_picking,
                    &colors,
                    &annotation_infos,
                    ent_context.world_from_entity,
                ));
            }
        }

        let mut line_batch = line_builder
            .batch(ent_path.to_string())
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, vector, origin, radius, color) in
            itertools::izip!(data.instance_keys, data.vectors, origins(), radii, colors,)
        {
            let vector: glam::Vec2 = vector.0.into();
            let origin: glam::Vec2 = origin.unwrap_or(Position2D::ZERO).0.into();
            let end = origin + vector;

            let segment = line_batch
                .add_segment_2d(origin, end)
                .radius(radius)
                .color(color)
                .flags(
                    LineStripFlags::FLAG_CAP_END_TRIANGLE
                        | LineStripFlags::FLAG_CAP_START_ROUND
                        | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS,
                )
                .picking_instance_id(PickingLayerInstanceId(instance_key.0));

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(instance_key) {
                segment.outline_mask_ids(*outline_mask_ids);
            }

            bounding_box.extend(origin.extend(0.0));
            bounding_box.extend(end.extend(0.0));
        }

        self.data
            .add_bounding_box(ent_path.hash(), bounding_box, ent_context.world_from_entity);
    }
}

// ---

struct Arrows2DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub vectors: &'a [Vector2D],
    pub origins: Option<&'a [Option<Position2D>]>,
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

impl IdentifiedViewSystem for Arrows2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Arrows2D".into()
    }
}

impl VisualizerSystem for Arrows2DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Arrows2D>()
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let num_arrows = super::entity_iterator::count_instances_in_archetype_views::<
            Arrows2DVisualizer,
            Arrows2D,
            8,
        >(ctx, query);

        if num_arrows == 0 {
            return Ok(Vec::new());
        }

        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx);
        line_builder.reserve_strips(num_arrows)?;
        line_builder.reserve_vertices(num_arrows * 2)?;
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype_pov1_comp6::<
            Arrows2DVisualizer,
            Arrows2D,
            Vector2D,
            Position2D,
            Color,
            Radius,
            Text,
            KeypointId,
            ClassId,
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx,
             ent_path,
             _ent_props,
             ent_context,
             (_time, _row_id),
             instance_keys,
             vectors,
             origins,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Arrows2DComponentData {
                    instance_keys,
                    vectors,
                    origins,
                    colors,
                    radii,
                    labels,
                    keypoint_ids,
                    class_ids,
                };
                self.process_data(&mut line_builder, query, &data, ent_path, ent_context);
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
