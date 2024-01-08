use re_entity_db::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_renderer::renderer::LineStripFlags;
use re_types::{
    archetypes::Arrows2D,
    components::{Position2D, Text, Vector2D},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
    VisualizableEntities, VisualizerSystem,
};

use super::{picking_id_from_instance_key, process_annotations, SpatialViewVisualizerData};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        entity_iterator::process_archetype_views, filter_visualizable_2d_entities, process_colors,
        process_radii, UiLabel, UiLabelTarget,
    },
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
        arch_view: &'a ArchetypeView<Arrows2D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<Vector2D>()?,
            arch_view.iter_optional_component::<Position2D>()?,
            arch_view.iter_optional_component::<Text>()?,
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
        );
        Ok(labels)
    }

    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<Arrows2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let annotation_infos =
            process_annotations::<Vector2D, Arrows2D>(query, arch_view, &ent_context.annotations)?;

        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let radii = process_radii(arch_view, ent_path)?;

        if arch_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_colors(arch_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                arch_view
                    .iter_instance_keys()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                arch_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            )?);
        }

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("arrows2d")
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let instance_keys = arch_view.iter_instance_keys();
        let pick_ids = arch_view
            .iter_instance_keys()
            .map(picking_id_from_instance_key);
        let vectors = arch_view.iter_required_component::<Vector2D>()?;
        let origins = arch_view.iter_optional_component::<Position2D>()?;

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, vector, origin, radius, color, pick_id) in
            itertools::izip!(instance_keys, vectors, origins, radii, colors, pick_ids)
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
                .picking_instance_id(pick_id);

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                segment.outline_mask_ids(*outline_mask_ids);
            }

            bounding_box.extend(origin.extend(0.0));
            bounding_box.extend(end.extend(0.0));
        }

        self.data
            .extend_bounding_box(bounding_box, ent_context.world_from_entity);

        Ok(())
    }
}

impl IdentifiedViewSystem for Arrows2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Arrows2D".into()
    }
}

impl VisualizerSystem for Arrows2DVisualizer {
    fn required_components(&self) -> ComponentNameSet {
        Arrows2D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Arrows2D::indicator().name()).collect()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn std::any::Any,
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
        process_archetype_views::<Arrows2DVisualizer, Arrows2D, { Arrows2D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.lines2d,
            |_ctx, ent_path, _ent_props, arch_view, ent_context| {
                self.process_arch_view(query, &arch_view, ent_path, ent_context)
            },
        )?;

        Ok(Vec::new()) // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
