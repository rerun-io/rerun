use re_entity_db::{EntityPath, InstancePathHash};
use re_renderer::LineDrawableBuilder;
use re_types::{
    archetypes::Boxes2D,
    components::{ClassId, Color, HalfSizes2D, InstanceKey, KeypointId, Position2D, Radius, Text},
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
    filter_visualizable_2d_entities, picking_id_from_instance_key,
    process_annotation_and_keypoint_slices, process_color_slice, process_radius_slice,
    SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

pub struct Boxes2DVisualizer {
    /// If the number of points in the batch is > max_labels, don't render box labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Boxes2DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 20,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Boxes2DVisualizer {
    fn process_labels<'a>(
        labels: &'a [Option<Text>],
        half_sizes: &'a [HalfSizes2D],
        centers: impl Iterator<Item = Position2D> + 'a,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        itertools::izip!(
            annotation_infos.iter(),
            half_sizes,
            centers,
            labels,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, half_size, center, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                let min = half_size.box_min(center);
                let max = half_size.box_max(center);
                label.map(|label| UiLabel {
                    text: label,
                    color: *color,
                    target: UiLabelTarget::Rect(egui::Rect::from_min_max(
                        egui::pos2(min.x, min.y),
                        egui::pos2(max.x, max.y),
                    )),
                    labeled_instance: *labeled_instance,
                })
            },
        )
    }

    fn process_data(
        &mut self,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        data: &Boxes2DComponentData<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) {
        let (annotation_infos, _) = process_annotation_and_keypoint_slices(
            query.latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.half_sizes.iter().map(|_| glam::Vec3::ZERO),
            &ent_context.annotations,
        );

        let centers = || {
            data.centers
                .as_ref()
                .map_or(
                    itertools::Either::Left(std::iter::repeat(&None).take(data.half_sizes.len())),
                    |data| itertools::Either::Right(data.iter()),
                )
                .map(|center| center.unwrap_or(Position2D::ZERO))
        };

        let radii = process_radius_slice(data.radii, data.half_sizes.len(), ent_path);
        let colors = process_color_slice(data.colors, ent_path, &annotation_infos);

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
                    labels,
                    data.half_sizes,
                    centers(),
                    &instance_path_hashes_for_picking,
                    &colors,
                    &annotation_infos,
                ));
            }
        }

        let mut line_batch = line_builder
            .batch("boxes2d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, half_size, center, radius, color) in itertools::izip!(
            data.instance_keys,
            data.half_sizes,
            centers(),
            radii,
            colors
        ) {
            let instance_hash = re_entity_db::InstancePathHash::instance(ent_path, *instance_key);

            let min = half_size.box_min(center);
            let max = half_size.box_max(center);
            bounding_box.extend(min.extend(0.0));
            bounding_box.extend(max.extend(0.0));

            let rectangle = line_batch
                .add_rectangle_outline_2d(
                    min,
                    glam::vec2(half_size.width(), 0.0),
                    glam::vec2(0.0, half_size.height()),
                )
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(*instance_key));
            if let Some(outline_mask_ids) = ent_context
                .highlight
                .instances
                .get(&instance_hash.instance_key)
            {
                rectangle.outline_mask_ids(*outline_mask_ids);
            }
        }

        self.data
            .add_bounding_box(ent_path.hash(), bounding_box, ent_context.world_from_entity);
    }
}

// ---

struct Boxes2DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub half_sizes: &'a [HalfSizes2D],
    pub centers: Option<&'a [Option<Position2D>]>,
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let num_boxes = super::entity_iterator::count_instances_in_archetype_views::<
            Boxes2DVisualizer,
            Boxes2D,
            9,
        >(ctx, query);

        if num_boxes == 0 {
            return Ok(Vec::new());
        }

        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        // Each box consists of 4 independent lines of 2 vertices each.
        line_builder.reserve_strips(num_boxes * 4)?;
        line_builder.reserve_vertices(num_boxes * 4 * 2)?;

        super::entity_iterator::process_archetype_pov1_comp6::<
            Boxes2DVisualizer,
            Boxes2D,
            HalfSizes2D,
            Position2D,
            Color,
            Radius,
            Text,
            re_types::components::KeypointId,
            re_types::components::ClassId,
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
             half_sizes,
             centers,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Boxes2DComponentData {
                    instance_keys,
                    half_sizes,
                    centers,
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
