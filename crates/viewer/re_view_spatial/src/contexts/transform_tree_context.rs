use std::sync::Arc;

use nohash_hasher::IntMap;
use vec1::smallvec_v1::SmallVec1;

use re_chunk_store::LatestAtQuery;
use re_log_types::EntityPathHash;
use re_tf::{TransformFrameId, TransformFrameIdHash};
use re_types::{archetypes, components::ImagePlaneDistance};
use re_view::{DataResultQuery as _, latest_at_with_blueprint_resolved_data};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextSystem, ViewContextSystemOncePerFrameResult,
};

use crate::caches::TransformDatabaseStoreCache;

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
        let mut transform_cache = caches
            .entry(|c: &mut TransformDatabaseStoreCache| c.lock_transform_cache(ctx.recording()));

        let transform_forest = re_tf::TransformForest::new(
            ctx.recording(),
            &mut transform_cache,
            &ctx.current_query(),
        );

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

        // Build a lookup table from entity paths to their transform frame id hashes.
        // Currently we don't keep it around during the frame, but we may do so in the future.
        let transform_frame_id_hash_to_entity_path_hashes =
            collect_entity_to_transform_frame_id_mapping(ctx, query);

        let latest_at_query = query.latest_at_query();

        let transform_infos_per_frame = self
            .transform_forest
            .transform_from_to(
                self.target_frame,
                transform_frame_id_hash_to_entity_path_hashes
                    .keys()
                    .copied(),
                &|transform_frame_id_hash| {
                    transform_frame_id_hash_to_entity_path_hashes
                        .get(&transform_frame_id_hash)
                        .map_or_else(
                            || 1.0,
                            |entity_path_hashes| {
                                let entity_path_hash = entity_path_hashes.first();
                                // TODO: what if there's several?
                                lookup_image_plane_distance(
                                    ctx,
                                    *entity_path_hash,
                                    &latest_at_query,
                                )
                            },
                        )
                },
                // Collect into Vec for simplicity, also bulk operating the transform loop seems like a good idea (perf citation needed!)
            )
            .collect::<Vec<_>>();

        self.transform_infos = {
            re_tracing::profile_scope!("transform info lookup");

            transform_infos_per_frame
                .into_iter()
                .filter_map(|(transform_frame_id_hash, transform_info)| {
                    transform_frame_id_hash_to_entity_path_hashes
                        .get(&transform_frame_id_hash)
                        .map(|entity_path_hashes| {
                            entity_path_hashes.iter().map(move |entity_path_hash| {
                                (*entity_path_hash, transform_info.clone())
                            })
                        })
                })
                .flatten()
                .collect()
        };
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
    entity_path_hash: EntityPathHash,
    latest_at_query: &LatestAtQuery,
) -> f32 {
    ctx.query_result
        .tree
        .lookup_result_by_path(entity_path_hash)
        .cloned()
        .map(|data_result| {
            data_result
                .latest_at_with_blueprint_resolved_data_for_component(
                    ctx,
                    latest_at_query,
                    archetypes::Pinhole::descriptor_image_plane_distance().component,
                )
                .get_mono_with_fallback::<ImagePlaneDistance>(
                    archetypes::Pinhole::descriptor_image_plane_distance().component,
                )
        })
        .unwrap_or_default()
        .into()
}

/// Transform frame _usually_ map 1:1 to entity paths,
/// but if requested by the user, several entity paths can share the same transform id.
type EntityToTransformFrameIdMapping = IntMap<TransformFrameIdHash, SmallVec1<[EntityPathHash; 1]>>;

/// Build a lookup table from entity paths to their transform frame id hashes.
fn collect_entity_to_transform_frame_id_mapping(
    ctx: &ViewContext<'_>,
    query: &re_viewer_context::ViewQuery<'_>,
) -> EntityToTransformFrameIdMapping {
    // This is blueprint dependent data and may also change over recording time,
    // making it non-trivial to cache.
    // That said, it changes rarely, so there's definitely an opportunity here for lazy updates!
    re_tracing::profile_function!();

    let latest_at_query = ctx.current_query();
    let transform_frame_id_descriptor = archetypes::CoordinateFrame::descriptor_frame_id();

    let mut transform_frame_id_hash_to_entity_path_hashes =
        EntityToTransformFrameIdMapping::default();

    for data_result in query.iter_all_data_results() {
        let query_shadowed_components = false;
        // TODO: Don't apply defaults here?
        let results = latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            &latest_at_query,
            data_result,
            [&transform_frame_id_descriptor],
            query_shadowed_components,
        );

        let frame_id = results
            .get_mono::<TransformFrameId>(&transform_frame_id_descriptor)
            .map_or_else(
                || TransformFrameIdHash::from_entity_path(&data_result.entity_path),
                |frame_id| TransformFrameIdHash::new(&frame_id),
            );

        match transform_frame_id_hash_to_entity_path_hashes.entry(frame_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SmallVec1::new(data_result.entity_path.hash()));
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                entry.into_mut().push(data_result.entity_path.hash());
            }
        }
    }

    transform_frame_id_hash_to_entity_path_hashes
}
