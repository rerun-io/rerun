use std::sync::Arc;

use nohash_hasher::IntMap;
use re_chunk_store::LatestAtQuery;
use re_log_types::EntityPathHash;
use re_tf::{TransformFrameId, TransformFrameIdHash};
use re_types::{archetypes, components::ImagePlaneDistance};
use re_view::{DataResultQuery as _, latest_at_with_blueprint_resolved_data};
use re_viewer_context::{
    DataResult, IdentifiedViewSystem, TransformDatabaseStoreCache, ViewContext, ViewContextSystem,
    ViewContextSystemOncePerFrameResult, typed_fallback_for,
};
use vec1::smallvec_v1::SmallVec1;

type FrameIdMapping = IntMap<TransformFrameIdHash, TransformFrameId>;

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
    target_frame_pinhole_root: Option<TransformFrameIdHash>,
    entity_transform_id_mapping: EntityTransformIdMapping,
    frame_id_mapping: Arc<FrameIdMapping>,
}

impl Default for TransformTreeContext {
    fn default() -> Self {
        Self {
            transform_forest: Arc::new(re_tf::TransformForest::default()),
            transform_infos: IntMap::default(),
            target_frame: TransformFrameIdHash::entity_path_hierarchy_root(),
            target_frame_pinhole_root: None,
            entity_transform_id_mapping: EntityTransformIdMapping::default(),
            frame_id_mapping: Arc::new(FrameIdMapping::default()),
        }
    }
}

#[derive(Clone, Default)]
struct EntityTransformIdMapping {
    /// Transform frame _usually_ map 1:1 to entity paths,
    /// but if requested by the user, several entity paths can share the same transform id.
    ///
    /// Contains also implicit transform frame ids.
    transform_frame_id_to_entity_path: IntMap<TransformFrameIdHash, SmallVec1<[EntityPathHash; 1]>>,

    /// Entity path to transform frame id mapping.
    ///
    /// Does *not* contain any implicit transform frame id.
    entity_path_to_transform_frame_id: IntMap<EntityPathHash, TransformFrameIdHash>,
}

impl IdentifiedViewSystem for TransformTreeContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

struct TransformTreeContextOncePerFrameResult {
    transform_forest: Arc<re_tf::TransformForest>,
    frame_id_mapping: Arc<FrameIdMapping>,
}

impl ViewContextSystem for TransformTreeContext {
    fn execute_once_per_frame(
        ctx: &re_viewer_context::ViewerContext<'_>,
    ) -> ViewContextSystemOncePerFrameResult {
        let caches = ctx.store_context.caches;
        let transform_cache = caches
            .entry(|c: &mut TransformDatabaseStoreCache| c.lock_transform_cache(ctx.recording()));

        let transform_forest =
            re_tf::TransformForest::new(ctx.recording(), &transform_cache, &ctx.current_query());

        let frame_ids = transform_cache
            .frame_id_registry()
            .iter_frame_ids()
            .map(|(k, v)| (*k, v.clone()));

        Box::new(TransformTreeContextOncePerFrameResult {
            transform_forest: Arc::new(transform_forest),
            frame_id_mapping: Arc::new(frame_ids.collect()),
        })
    }

    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
        static_execution_result: &ViewContextSystemOncePerFrameResult,
    ) {
        let static_execution_result = static_execution_result
            .downcast_ref::<TransformTreeContextOncePerFrameResult>()
            .expect("Unexpected static execution result type");

        self.transform_forest = static_execution_result.transform_forest.clone();
        self.frame_id_mapping = static_execution_result.frame_id_mapping.clone();

        // Build a lookup table from entity paths to their transform frame id hashes.
        // Currently, we don't keep it around during the frame, but we may do so in the future.
        self.entity_transform_id_mapping = EntityTransformIdMapping::new(ctx, query);

        // Target frame is the coordinate frame of the space origin entity.
        self.target_frame = self.transform_frame_id_for(query.space_origin.hash());

        let latest_at_query = query.latest_at_query();

        // Add overrides to the transform frame id map so we can get back the id for errors.
        for data_result in query.iter_all_data_results() {
            let result = re_view::query_overrides(
                ctx.viewer_ctx,
                data_result,
                [archetypes::CoordinateFrame::descriptor_frame().component],
            );

            let Some(batch) =
                result.component_batch(archetypes::CoordinateFrame::descriptor_frame().component)
            else {
                continue;
            };

            for frame in batch {
                let frame_hash = TransformFrameIdHash::new(&frame);
                if !self.frame_id_mapping.contains_key(&frame_hash) {
                    // As overrides are local to this view we need to clone the whole map to add new hashes.
                    Arc::make_mut(&mut self.frame_id_mapping).insert(frame_hash, frame);
                }
            }
        }

        let transform_infos_per_frame = self
            .transform_forest
            .transform_from_to(
                self.target_frame,
                self.entity_transform_id_mapping
                    .transform_frame_id_to_entity_path
                    .keys()
                    .copied(),
                &|transform_frame_id_hash| {
                    self.entity_transform_id_mapping
                        .transform_frame_id_to_entity_path
                        .get(&transform_frame_id_hash)
                        .map_or_else(
                            || 1.0,
                            |entity_paths| {
                                lookup_image_plane_distance(ctx, entity_paths, &latest_at_query)
                                    as f64
                            },
                        )
                },
                // Collect into Vec for simplicity, also bulk operating on the transform loop seems like a good idea (perf citation needed!)
            )
            .collect::<Vec<_>>();

        self.transform_infos = {
            re_tracing::profile_scope!("transform info lookup");

            transform_infos_per_frame
                .into_iter()
                .filter_map(|(transform_frame_id_hash, transform_info)| {
                    self.entity_transform_id_mapping
                        .transform_frame_id_to_entity_path
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

        self.target_frame_pinhole_root = self
            .transform_forest
            .root_from_frame(self.target_frame)
            .and_then(|info| {
                self.transform_forest
                    .pinhole_tree_root_info(info.tree_root())
                    .map(|_pinhole_info| info.tree_root())
            });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformTreeContext {
    /// Returns a transform info describing how to get to the view's target frame for the given entity.
    #[inline]
    pub fn target_from_entity_path(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> Option<&Result<re_tf::TransformInfo, re_tf::TransformFromToError>> {
        self.transform_infos.get(&entity_path_hash)
    }

    /// Returns the properties of the pinhole tree root at given frame if the entity's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info(
        &self,
        frame: TransformFrameIdHash,
    ) -> Option<&re_tf::PinholeTreeRoot> {
        self.transform_forest.pinhole_tree_root_info(frame)
    }

    /// Returns the target frame, also known as the space origin.
    #[inline]
    pub fn target_frame(&self) -> TransformFrameIdHash {
        self.target_frame
    }

    /// Iff the target frame has a pinhole tree root, returns its transform frame id.
    ///
    /// If this is `Some`, then this is either the frame target frame itself or one of its ancestors.
    /// `Some` implies that the view as a whole is two dimensional.
    #[inline]
    pub fn target_frame_pinhole_root(&self) -> Option<TransformFrameIdHash> {
        self.target_frame_pinhole_root
    }

    /// Returns the transform frame id for a given entity path.
    #[inline]
    pub fn transform_frame_id_for(&self, entity_path: EntityPathHash) -> TransformFrameIdHash {
        self.entity_transform_id_mapping
            .entity_path_to_transform_frame_id
            .get(&entity_path)
            .copied()
            .unwrap_or_else(|| TransformFrameIdHash::from_entity_path_hash(entity_path))
    }

    /// Looks up a frame ID by its hash.
    ///
    /// Returns `None` if the frame id hash was never encountered.
    #[inline]
    pub fn lookup_frame_id(
        &self,
        frame_id_hash: TransformFrameIdHash,
    ) -> Option<&TransformFrameId> {
        self.frame_id_mapping.get(&frame_id_hash)
    }

    /// Formats a frame ID hash as a human-readable string.
    ///
    /// Returns the frame name if known, otherwise returns a debug representation of the hash.
    #[inline]
    pub fn format_frame(&self, frame_id_hash: TransformFrameIdHash) -> String {
        self.lookup_frame_id(frame_id_hash)
            .map_or_else(|| format!("{frame_id_hash:?}"), ToString::to_string)
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    entity_path_hashes: &SmallVec1<[EntityPathHash; 1]>,
    latest_at_query: &LatestAtQuery,
) -> f32 {
    // If there's several entity paths (with pinhole cameras) for the same transform id,
    // we don't know which camera plane to use.
    //
    // That's rather strange, but a scene can be set up for this to happen!
    // Unfortunately it's also really hard to log a warning or anything at this point since
    // we don't know the full entity path names.
    //
    // We're letting it slide for now since it's kinda hard to get into that situation.
    let entity_path_hash = *entity_path_hashes.first();

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

impl EntityTransformIdMapping {
    /// Build a lookup table from entity paths to their transform frame id hashes.
    fn new(ctx: &ViewContext<'_>, query: &re_viewer_context::ViewQuery<'_>) -> Self {
        // This is blueprint-dependent data and may also change over recording time,
        // making it non-trivial to cache.
        // That said, it rarely changes, so there's definitely an opportunity here for lazy updates!
        re_tracing::profile_function!();

        let mut mapping = Self::default();

        // Determine mapping for all _visible_ data results.
        for data_result in query.iter_all_data_results() {
            mapping.determine_frame_id_mapping_for(ctx, data_result);
        }

        // The origin entity may not be visible, make sure it's included.
        if !mapping
            .entity_path_to_transform_frame_id
            .contains_key(&query.space_origin.hash())
            && let Some(origin_data_result) = ctx
                .query_result
                .tree
                .lookup_result_by_path(query.space_origin.hash())
        {
            mapping.determine_frame_id_mapping_for(ctx, origin_data_result);
        }

        mapping
    }

    fn determine_frame_id_mapping_for(&mut self, ctx: &ViewContext<'_>, data_result: &DataResult) {
        let latest_at_query = ctx.current_query();

        let transform_frame_id_component =
            archetypes::CoordinateFrame::descriptor_frame().component;

        let query_shadowed_components = false;
        let results = latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            &latest_at_query,
            data_result,
            [transform_frame_id_component],
            query_shadowed_components,
        );

        let frame_id = results
            .get_mono::<TransformFrameId>(transform_frame_id_component)
            .map_or_else(
                || {
                    let fallback = TransformFrameIdHash::from_entity_path(&data_result.entity_path);
                    // Make sure this is the same as the fallback provider (which is a lot slower to run)
                    debug_assert_eq!(
                        TransformFrameIdHash::new(&typed_fallback_for::<TransformFrameId>(
                            &ctx.query_context(data_result, &latest_at_query),
                            transform_frame_id_component
                        )),
                        fallback
                    );
                    fallback
                },
                |frame_id| TransformFrameIdHash::new(&frame_id),
            );

        let entity_path_hash = data_result.entity_path.hash();

        match self.transform_frame_id_to_entity_path.entry(frame_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SmallVec1::new(entity_path_hash));
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                entry.into_mut().push(entity_path_hash);
            }
        }
        self.entity_path_to_transform_frame_id
            .insert(entity_path_hash, frame_id);
    }
}
