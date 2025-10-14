use std::sync::Arc;

use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, EntityPathHash};
use re_types::{archetypes, components::ImagePlaneDistance};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextSystem, ViewContextSystemOncePerFrameResult,
};

use crate::{caches::TransformDatabaseStoreCache, visualizers::CamerasVisualizer};

/// Provides a transform tree for the view & time it operates on.
///
/// Will do the necessary bulk processing of transform information and make it available
/// for quick lookups by visualizers.
#[derive(Clone)]
pub struct TransformTreeContext {
    transform_forest: Arc<re_tf::TransformForest>,
    transform_infos:
        IntMap<EntityPathHash, Result<re_tf::TransformInfo, re_tf::TransformFromToError>>,
    target_path: EntityPathHash,
}

impl Default for TransformTreeContext {
    fn default() -> Self {
        Self {
            transform_forest: Arc::new(re_tf::TransformForest::default()),
            transform_infos: IntMap::default(),
            target_path: EntityPath::root().hash(),
        }
    }
}

impl IdentifiedViewSystem for TransformTreeContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

struct TransformTreeContextOncePerFrameResult {
    transform_forest: Arc<re_tf::TransformForest>,
}

impl ViewContextSystem for TransformTreeContext {
    fn execute_once_per_frame(
        ctx: &re_viewer_context::ViewerContext<'_>,
    ) -> ViewContextSystemOncePerFrameResult {
        let caches = ctx.store_context.caches;
        let transform_cache = caches.entry(|c: &mut TransformDatabaseStoreCache| {
            c.read_lock_transform_cache(ctx.recording())
        });

        let transform_forest =
            re_tf::TransformForest::new(ctx.recording(), &transform_cache, &ctx.current_query());

        Box::new(TransformTreeContextOncePerFrameResult {
            transform_forest: Arc::new(transform_forest),
        })
    }

    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
        static_execution_result: &ViewContextSystemOncePerFrameResult,
    ) {
        self.target_path = query.space_origin.hash();
        self.transform_forest = static_execution_result
            .downcast_ref::<TransformTreeContextOncePerFrameResult>()
            .expect("Unexpected static execution result type")
            .transform_forest
            .clone();

        let latest_at_query = query.latest_at_query();

        self.transform_infos = self
            .transform_forest
            .transform_from_to(
                self.target_path,
                query.iter_all_entities().map(|e| e.hash()),
                &|entity_path| lookup_image_plane_distance(ctx, entity_path, &latest_at_query),
            )
            .collect();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformTreeContext {
    /// Transform info to get from the entity's transform to the origin transform frame.
    #[inline]
    pub fn transform_info_for_entity(
        &self,
        entity_path: EntityPathHash,
    ) -> Option<&re_tf::TransformInfo> {
        self.transform_infos.get(&entity_path)?.as_ref().ok()
    }

    /// Returns the properties of the pinhole tree root at the given entity path if the entity's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info(
        &self,
        entity_path: EntityPathHash,
    ) -> Option<&re_tf::PinholeTreeRoot> {
        self.transform_forest.pinhole_tree_root_info(entity_path)
    }

    /// Returns the target path, also known as the space origin.
    #[inline]
    pub fn target_path(&self) -> EntityPathHash {
        self.target_path
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    entity_path: EntityPathHash,
    latest_at_query: &LatestAtQuery,
) -> f32 {
    let data_result_tree = &ctx.query_result.tree;

    data_result_tree
        .lookup_result_by_path(entity_path)
        .cloned()
        .map(|data_result| {
            data_result
                .latest_at_with_blueprint_resolved_data_for_component(
                    ctx,
                    latest_at_query,
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
