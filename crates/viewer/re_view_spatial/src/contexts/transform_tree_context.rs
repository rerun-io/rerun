use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, EntityPathHash};
use re_types::{archetypes, components::ImagePlaneDistance};
use re_view::DataResultQuery as _;
use re_viewer_context::{DataResultTree, IdentifiedViewSystem, ViewContext, ViewContextSystem};

use crate::visualizers::CamerasVisualizer;

#[derive(Clone, Default)]
pub struct TransformTreeContext(re_tf::TransformTree);

impl IdentifiedViewSystem for TransformTreeContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

impl ViewContextSystem for TransformTreeContext {
    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        let query_result = ctx.viewer_ctx.lookup_query_result(query.view_id);
        let data_result_tree = &query_result.tree;

        self.0.set_reference_path(query.space_origin.clone());

        let time_query = query.latest_at_query();
        self.0
            .execute(ctx.recording(), &time_query, &|entity_path| {
                lookup_image_plane_distance(ctx, data_result_tree, entity_path, &time_query)
            });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformTreeContext {
    #[inline]
    pub fn transform_info_for_entity(
        &self,
        entity_path: EntityPathHash,
    ) -> Option<&re_tf::TransformInfo> {
        self.0.transform_info_for_entity(entity_path)
    }

    #[inline]
    pub fn reference_path(&self) -> &EntityPath {
        self.0.reference_path()
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    data_result_tree: &DataResultTree,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> f32 {
    data_result_tree
        .lookup_result_by_path(entity_path)
        .cloned()
        .map(|data_result| {
            data_result
                .latest_at_with_blueprint_resolved_data_for_component(
                    ctx,
                    query,
                    &archetypes::Pinhole::descriptor_image_plane_distance(),
                )
                .get_mono_with_fallback::<ImagePlaneDistance>(
                    &archetypes::Pinhole::descriptor_image_plane_distance(),
                    &CamerasVisualizer::default(),
                )
        })
        .unwrap_or_default()
        .into()
}
