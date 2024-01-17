use re_entity_db::EntityPath;
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::Boxes3D,
    components::{HalfSizes3D, Position3D, Rotation3D},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{UiLabel, UiLabelTarget},
};

use super::{
    entity_iterator::process_archetype_views, filter_visualizable_3d_entities,
    picking_id_from_instance_key, process_annotations, process_colors, process_labels,
    process_radii, SpatialViewVisualizerData,
};

pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

impl Boxes3DVisualizer {
    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<Boxes3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let annotation_infos = process_annotations::<HalfSizes3D, Boxes3D>(
            query,
            arch_view,
            &ent_context.annotations,
        )?;

        let instance_keys = arch_view.iter_instance_keys();
        let half_sizes = arch_view.iter_required_component::<HalfSizes3D>()?;
        let positions = arch_view
            .iter_optional_component::<Position3D>()?
            .map(|position| position.unwrap_or(Position3D::ZERO));
        let rotation = arch_view
            .iter_optional_component::<Rotation3D>()?
            .map(|position| position.unwrap_or(Rotation3D::IDENTITY));
        let radii = process_radii(arch_view, ent_path)?;
        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let labels = process_labels(arch_view, &annotation_infos)?;

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("boxes3d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_entity)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();

        for (instance_key, half_extent, position, rotation, radius, color, label) in itertools::izip!(
            instance_keys,
            half_sizes,
            positions,
            rotation,
            radii,
            colors,
            labels
        ) {
            let instance_hash = re_entity_db::InstancePathHash::instance(ent_path, instance_key);

            bounding_box.extend(half_extent.box_min(position));
            bounding_box.extend(half_extent.box_max(position));

            let position = position.into();

            let box3d = line_batch
                .add_box_outline_from_transform(glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::from(half_extent) * 2.0,
                    rotation.into(),
                    position,
                ))
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(instance_key));
            if let Some(outline_mask_ids) = ent_context
                .highlight
                .instances
                .get(&instance_hash.instance_key)
            {
                box3d.outline_mask_ids(*outline_mask_ids);
            }

            if let Some(text) = label {
                self.0.ui_labels.push(UiLabel {
                    text,
                    color,
                    target: UiLabelTarget::Position3D(
                        ent_context.world_from_entity.transform_point3(position),
                    ),
                    labeled_instance: instance_hash,
                });
            }
        }

        self.0
            .add_bounding_box(ent_path.hash(), bounding_box, ent_context.world_from_entity);

        Ok(())
    }
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn required_components(&self) -> ComponentNameSet {
        Boxes3D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Boxes3D::indicator().as_ref().name()).collect()
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
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        process_archetype_views::<Boxes3DVisualizer, Boxes3D, { Boxes3D::NUM_COMPONENTS }, _>(
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
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
