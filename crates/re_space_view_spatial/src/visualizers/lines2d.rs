use re_entity_db::{EntityPath, InstancePathHash};
use re_renderer::LineDrawableBuilder;
use re_types::{
    archetypes::LineStrips2D,
    components::{ClassId, Color, InstanceKey, KeypointId, LineStrip2D, Radius, Text},
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
    filter_visualizable_2d_entities, process_annotation_and_keypoint_slices, process_color_slice,
    process_radius_slice, SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

pub struct Lines2DVisualizer {
    /// If the number of arrows in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines2DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Lines2DVisualizer {
    fn process_labels<'a>(
        strips: &'a [LineStrip2D],
        labels: &'a [Option<Text>],
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        itertools::izip!(
            annotation_infos.iter(),
            strips,
            labels,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, strip, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                match (strip, label) {
                    (strip, Some(label)) => {
                        let midpoint = strip
                            .0
                            .iter()
                            .copied()
                            .map(glam::Vec2::from)
                            .sum::<glam::Vec2>()
                            / (strip.0.len() as f32);

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
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        data: &Lines2DComponentData<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let (annotation_infos, _) = process_annotation_and_keypoint_slices(
            query.latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.strips.iter().map(|_| glam::Vec3::ZERO),
            &ent_context.annotations,
        );

        let radii = process_radius_slice(data.radii, data.strips.len(), ent_path);
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
                    data.strips,
                    labels,
                    &instance_path_hashes_for_picking,
                    &colors,
                    &annotation_infos,
                ));
            }
        }

        line_builder.reserve_strips(data.strips.len())?;
        line_builder.reserve_vertices(data.strips.iter().map(|s| s.0.len()).sum())?;

        let mut line_batch = line_builder
            .batch(ent_path.to_string())
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();
        for (instance_key, strip, radius, color) in
            itertools::izip!(data.instance_keys, data.strips, radii, colors)
        {
            let lines = line_batch
                .add_strip_2d(strip.0.iter().copied().map(Into::into))
                .color(color)
                .radius(radius)
                .picking_instance_id(re_renderer::PickingLayerInstanceId(instance_key.0));

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(instance_key) {
                lines.outline_mask_ids(*outline_mask_ids);
            }

            for p in &strip.0 {
                bounding_box.extend(glam::vec3(p.x(), p.y(), 0.0));
            }
        }

        self.data
            .add_bounding_box(ent_path.hash(), bounding_box, ent_context.world_from_entity);

        Ok(())
    }
}

// ---

struct Lines2DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub strips: &'a [LineStrip2D],
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        // Counting all lines (strips and vertices) ahead of time is a bit expensive since we need to do a full query for this.
        // We choose a semi-dynamic approach here, where we reserve on every new line batch.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype_pov1_comp5::<
            Lines2DVisualizer,
            LineStrips2D,
            LineStrip2D,
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
             strips,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Lines2DComponentData {
                    instance_keys,
                    strips,
                    colors,
                    radii,
                    labels,
                    keypoint_ids,
                    class_ids,
                };
                self.process_data(&mut line_builder, query, &data, ent_path, ent_context)
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
