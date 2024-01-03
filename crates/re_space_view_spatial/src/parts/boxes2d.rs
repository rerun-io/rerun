use re_data_store::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::Boxes2D,
    components::{HalfSizes2D, Position2D, Text},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
    VisualizableEntities,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{UiLabel, UiLabelTarget},
    view_kind::SpatialSpaceViewKind,
};

use super::{
    entity_iterator::process_archetype_views, filter_visualizable_2d_entities,
    picking_id_from_instance_key, process_annotations, process_colors, process_radii,
    SpatialViewPartData,
};

pub struct Boxes2DPart {
    /// If the number of points in the batch is > max_labels, don't render box labels.
    pub max_labels: usize,
    pub data: SpatialViewPartData,
}

impl Default for Boxes2DPart {
    fn default() -> Self {
        Self {
            max_labels: 20,
            data: SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Boxes2DPart {
    fn process_labels<'a>(
        arch_view: &'a ArchetypeView<Boxes2D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<HalfSizes2D>()?,
            arch_view.iter_optional_component::<Position2D>()?,
            arch_view.iter_optional_component::<Text>()?,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, half_size, center, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                let center = center.unwrap_or(Position2D::ZERO);
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
        );
        Ok(labels)
    }

    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<Boxes2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let annotation_infos = process_annotations::<HalfSizes2D, Boxes2D>(
            query,
            arch_view,
            &ent_context.annotations,
        )?;

        let instance_keys = arch_view.iter_instance_keys();
        let half_sizes = arch_view.iter_required_component::<HalfSizes2D>()?;
        let positions = arch_view
            .iter_optional_component::<Position2D>()?
            .map(|position| position.unwrap_or(Position2D::ZERO));
        let radii = process_radii(arch_view, ent_path)?;
        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;

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
            )?);
        }

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("boxes2d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        for (instance_key, half_size, position, radius, color) in
            itertools::izip!(instance_keys, half_sizes, positions, radii, colors)
        {
            let instance_hash = re_data_store::InstancePathHash::instance(ent_path, instance_key);

            let min = half_size.box_min(position);
            let max = half_size.box_max(position);

            self.data.extend_bounding_box(
                macaw::BoundingBox {
                    min: min.extend(0.),
                    max: max.extend(0.),
                },
                ent_context.world_from_entity,
            );

            let rectangle = line_batch
                .add_rectangle_outline_2d(
                    min,
                    glam::vec2(half_size.width(), 0.0),
                    glam::vec2(0.0, half_size.height()),
                )
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(instance_key));
            if let Some(outline_mask_ids) = ent_context
                .highlight
                .instances
                .get(&instance_hash.instance_key)
            {
                rectangle.outline_mask_ids(*outline_mask_ids);
            }
        }

        Ok(())
    }
}

impl IdentifiedViewSystem for Boxes2DPart {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes2D".into()
    }
}

impl ViewPartSystem for Boxes2DPart {
    fn required_components(&self) -> ComponentNameSet {
        Boxes2D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Boxes2D::indicator().name()).collect()
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
        process_archetype_views::<Boxes2DPart, Boxes2D, { Boxes2D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.box2d,
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
