use re_components::{ColorRGBA, Component as _, InstanceKey, LineStrip3D, Radius};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, ScenePart, SceneQuery, SpaceViewHighlights, ViewerContext,
};

use crate::{
    scene::{
        contexts::{SpatialSceneContext, SpatialSceneEntityContext},
        parts::entity_iterator::process_entity_views,
    },
    SpatialSpaceView,
};

use super::{instance_key_to_picking_id, SpatialScenePartData, SpatialSpaceViewState};

#[derive(Default)]
pub struct Lines3DPart(SpatialScenePartData);

impl Lines3DPart {
    fn process_entity_view(
        &mut self,
        _query: &SceneQuery<'_>,
        ent_view: &EntityView<LineStrip3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("lines 3D")
            .depth_offset(ent_context.depth_offset)
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       strip: LineStrip3D,
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
                .add_strip(strip.0.into_iter().map(|v| v.into()))
                .color(color)
                .radius(radius)
                .picking_instance_id(instance_key_to_picking_id(instance_key));

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
                    .map(|pt| pt.into())
            }),
            ent_context.world_from_obj,
        );

        Ok(())
    }
}

impl ScenePart<SpatialSpaceView> for Lines3DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            LineStrip3D::name(),
            InstanceKey::name(),
            ColorRGBA::name(),
            Radius::name(),
        ]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Lines3DPart");

        process_entity_views::<LineStrip3D, 4, _>(
            ctx,
            query,
            scene_context,
            highlights,
            scene_context.depth_offsets.points,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
                scene_context
                    .num_3d_primitives
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
            },
        );

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.0)
    }
}
