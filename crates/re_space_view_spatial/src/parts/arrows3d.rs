use re_components::{Arrow3D, ColorRGBA, InstanceKey, Label, Radius};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::{renderer::LineStripFlags, Size};
use re_types::Loggable as _;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

use super::{picking_id_from_instance_key, SpatialViewPartData};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::entity_iterator::process_entity_views,
};

#[derive(Default)]
pub struct Arrows3DPart(SpatialViewPartData);

impl Arrows3DPart {
    fn process_entity_view(
        &mut self,
        _query: &ViewQuery<'_>,
        ent_view: &EntityView<Arrow3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("arrows")
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let mut bounding_box = macaw::BoundingBox::nothing();

        ent_view.visit4(
            |instance_key: InstanceKey,
             arrow: Arrow3D,
             color: Option<ColorRGBA>,
             radius: Option<Radius>,
             _label: Option<Label>| {
                // TODO(andreas): support labels
                // TODO(andreas): support class ids for arrows
                let annotation_info = ent_context
                    .annotations
                    .class_description(None)
                    .annotation_info();
                let color =
                    annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
                //let label = annotation_info.label(label);

                let re_components::Arrow3D { origin, vector } = arrow;

                let vector = glam::Vec3::from(vector);
                let origin = glam::Vec3::from(origin);

                bounding_box.extend(vector);
                bounding_box.extend(origin);

                let radius = radius.map_or(Size::AUTO, |r| Size(r.0));
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
                    .picking_instance_id(picking_id_from_instance_key(instance_key));

                if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                    segment.outline_mask_ids(*outline_mask_ids);
                }
            },
        )?;

        self.0
            .extend_bounding_box(bounding_box, ent_context.world_from_obj);

        Ok(())
    }
}

impl ViewPartSystem for Arrows3DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            Arrow3D::name(),
            InstanceKey::name(),
            ColorRGBA::name(),
            Radius::name(),
            Label::name(),
        ]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_scope!("Arrows3DPart");

        process_entity_views::<Arrow3D, 5, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
                ent_context
                    .counter
                    .num_3d_primitives
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
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
