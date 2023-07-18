use re_components::{ColorRGBA, Component as _, InstanceKey, LineStrip2D, Radius};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::entity_iterator::process_entity_views,
    view_kind::SpatialSpaceViewKind,
};

use super::{picking_id_from_instance_key, SpatialViewPartData};

pub struct Lines2DPart(SpatialViewPartData);

impl Default for Lines2DPart {
    fn default() -> Self {
        Self(SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)))
    }
}

impl Lines2DPart {
    fn process_entity_view(
        &mut self,
        _query: &ViewQuery<'_>,
        ent_view: &EntityView<LineStrip2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("lines 2d")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       strip: LineStrip2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            // TODO(andreas): support class ids for lines
            let annotation_info = ent_context
                .annotations
                .class_description(None)
                .annotation_info();
            let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let lines = line_batch
                .add_strip_2d(strip.0.into_iter().map(|v| v.into()))
                .color(color)
                .radius(radius)
                .picking_instance_id(picking_id_from_instance_key(instance_key));

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                lines.outline_mask_ids(*outline_mask_ids);
            }
        };

        ent_view.visit3(visitor)?;

        self.0.extend_bounding_box_with_points(
            ent_view.iter_primary()?.flat_map(|strip| {
                strip
                    .map_or(Vec::new(), |strip| strip.0)
                    .into_iter()
                    .map(|pt| glam::vec3(pt.x(), pt.y(), 0.0))
            }),
            ent_context.world_from_obj,
        );

        Ok(())
    }
}

impl ViewPartSystem for Lines2DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            LineStrip2D::name(),
            InstanceKey::name(),
            ColorRGBA::name(),
            Radius::name(),
        ]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_scope!("Lines2DPart");

        process_entity_views::<LineStrip2D, 4, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.lines2d,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
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
