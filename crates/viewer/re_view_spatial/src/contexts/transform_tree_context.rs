use std::sync::Arc;

use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_log_types::EntityPathHash;
use re_tf::TransformFrameIdHash;
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
    target_frame: TransformFrameIdHash,
}

impl Default for TransformTreeContext {
    fn default() -> Self {
        Self {
            transform_forest: Arc::new(re_tf::TransformForest::default()),
            transform_infos: IntMap::default(),
            target_frame: TransformFrameIdHash::entity_path_hierarchy_root(),
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
        self.target_frame = TransformFrameIdHash::from_entity_path(query.space_origin);
        self.transform_forest = static_execution_result
            .downcast_ref::<TransformTreeContextOncePerFrameResult>()
            .expect("Unexpected static execution result type")
            .transform_forest
            .clone();

        let latest_at_query = query.latest_at_query();

        self.transform_infos = self
            .transform_forest
            .transform_from_to(
                self.target_frame,
                query
                    .iter_all_entities()
                    // TODO(RR-2497): Perform EntityPath <-> TransformFrameIdHash lookups here.
                    .map(TransformFrameIdHash::from_entity_path),
                &|transform_frame_id_hash| {
                    lookup_image_plane_distance(ctx, transform_frame_id_hash, &latest_at_query)
                },
            )
            .map(|(transform_frame_id_hash, transform_info)| {
                (
                    // TODO(RR-2497): Perform EntityPath <-> TransformFrameIdHash lookups here?
                    transform_frame_id_hash.as_entity_path_hash(),
                    transform_info,
                )
            })
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

    /// Returns the properties of the pinhole tree root at given frame if the entity's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info(
        &self,
        frame: TransformFrameIdHash,
    ) -> Option<&re_tf::PinholeTreeRoot> {
        self.transform_forest.pinhole_tree_root_info(frame)
    }

    /// Returns the properties of the pinhole tree root at given frame if the entity's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info_for_entity(
        &self,
        entity_path: EntityPathHash,
    ) -> Option<&re_tf::PinholeTreeRoot> {
        // TODO(RR-2497): Perform EntityPath <-> TransformFrameIdHash lookups here.
        let frame = TransformFrameIdHash::from_entity_path_hash(entity_path);
        self.transform_forest.pinhole_tree_root_info(frame)
    }

    /// Returns the target frame, also known as the space origin.
    #[inline]
    pub fn target_frame(&self) -> TransformFrameIdHash {
        self.target_frame
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    transform_frame_id_hash: TransformFrameIdHash,
    latest_at_query: &LatestAtQuery,
) -> f32 {
    let data_result_tree = &ctx.query_result.tree;

    // If this IS entity path derived, we may find a data result. Otherwise we won't which is just fine.
    let entity_path_hash = transform_frame_id_hash.as_entity_path_hash();

    data_result_tree
        .lookup_result_by_path(entity_path_hash)
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
