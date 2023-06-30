use re_components::{Component as _, Transform3D};
use re_viewer_context::{
    ArchetypeDefinition, ScenePart, SceneQuery, SpaceViewHighlights, ViewerContext,
};

use crate::{axis_lines::add_axis_lines, scene::contexts::SpatialSceneContext, SpatialSpaceView};

use super::{SpatialScenePartData, SpatialSpaceViewState};

#[derive(Default)]
pub struct TransformGizmoPart(SpatialScenePartData);

impl ScenePart<SpatialSpaceView> for TransformGizmoPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Transform3D::name()]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        _highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("TransformGizmoPart");

        let mut line_builder = scene_context.shared_render_builders.lines();

        let store = &ctx.store_db.entity_db.data_store;
        let latest_at_query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);
        for (ent_path, _props) in query.iter_entities() {
            // Given how simple transform gizmos are it would be nice to put them all into a single line batch.
            // However, we can set object picking ids only per batch.

            // Transforms are mono-components.
            if let Some(_transform) =
                store.query_latest_component::<Transform3D>(ent_path, &latest_at_query)
            {
                // Apply the transform _and_ the parent transform, but if we're at a pinhole camera ignore that part.
                let Some(world_from_obj) = scene_context.transforms.
                    reference_from_entity_ignore_pinhole(ent_path, store, &latest_at_query) else {
                    continue;
                };

                // Use unit size for axis lines.
                add_axis_lines(&mut line_builder, world_from_obj, Some(ent_path), 0.03);
            }
        }

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.0)
    }
}
