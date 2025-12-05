use std::sync::Arc;

use nohash_hasher::IntMap;
use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, EntityPathHash};
use re_tf::{TransformFrameId, TransformFrameIdHash, TreeTransform};
use re_types::components::ImagePlaneDistance;
use re_types::{ArchetypeName, archetypes, blueprint};
use re_view::{
    DataResultQuery as _, HybridLatestAtResults, latest_at_with_blueprint_resolved_data,
};
use re_viewer_context::{
    IdentifiedViewSystem, TransformDatabaseStoreCache, ViewContext, ViewContextSystem,
    ViewContextSystemOncePerFrameResult, typed_fallback_for,
};
use re_viewport_blueprint::ViewProperty;
use vec1::smallvec_v1::SmallVec1;

type FrameIdMapping = IntMap<TransformFrameIdHash, TransformFrameId>;

/// Details on how to transform an entity (!) to the origin of the view's space.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformInfo {
    /// Root frame this transform belongs to.
    ///
    /// ⚠️ This is the root of the tree this transform belongs to,
    /// not necessarily what the transform transforms into.
    ///
    /// Implementation note:
    /// We could add target and maybe even source to this, but we want to keep this struct small'ish.
    /// On that note, it may be good to split this in the future, as most of the time we're only interested in the
    /// source->target affine transform.
    root: TransformFrameIdHash,

    /// The transform from this frame to the target's space.
    ///
    /// ⚠️ Does not include per instance poses! ⚠️
    /// Include 3D-from-2D / 2D-from-3D pinhole transform if present.
    target_from_source: glam::DAffine3,

    /// List of transforms per instance including poses.
    ///
    /// If no poses are present, this is always the same as [`Self::target_from_source`].
    /// (also implying that in this case there is only a single element).
    /// If there are poses, there may be more than one element.
    target_from_instances: SmallVec1<[glam::DAffine3; 1]>,
}

impl TransformInfo {
    fn new(tree_transform: &TreeTransform, mut pose_transforms: Vec<glam::DAffine3>) -> Self {
        for pose_transforms in &mut pose_transforms {
            *pose_transforms = tree_transform.target_from_source * *pose_transforms;
        }
        let target_from_instances = SmallVec1::try_from_vec(pose_transforms)
            .unwrap_or_else(|_| SmallVec1::new(tree_transform.target_from_source));

        Self {
            root: tree_transform.root,
            target_from_source: tree_transform.target_from_source,
            target_from_instances,
        }
    }

    /// Returns the root frame of the tree this transform belongs to.
    ///
    /// This is **not** necessarily the transform's target frame.
    #[inline]
    pub fn tree_root(&self) -> TransformFrameIdHash {
        self.root
    }

    /// Warns that multiple transforms on an entity are not supported.
    #[inline]
    fn warn_on_per_instance_transform(&self, entity_name: &EntityPath, archetype: ArchetypeName) {
        if self.target_from_instances.len() > 1 {
            re_log::warn_once!(
                "There are multiple poses for entity {entity_name:?}'s transform frame. {archetype:?} supports only one transform per entity. Using the first one."
            );
        }
    }

    /// Returns the first instance transform and warns if there are multiple.
    #[inline]
    pub fn single_transform_required_for_entity(
        &self,
        entity_name: &EntityPath,
        archetype: ArchetypeName,
    ) -> glam::DAffine3 {
        self.warn_on_per_instance_transform(entity_name, archetype);
        *self.target_from_instances.first()
    }

    /// Returns the target from instance transforms.
    #[inline]
    pub fn target_from_instances(&self) -> &SmallVec1<[glam::DAffine3; 1]> {
        &self.target_from_instances
    }
}

/// Provides a transform tree for the view & time it operates on.
///
/// Will do the necessary bulk processing of transform information and make it available
/// for quick lookups by visualizers.
#[derive(Clone)]
pub struct TransformTreeContext {
    transform_forest: Arc<re_tf::TransformForest>,
    transform_infos: IntMap<EntityPathHash, Result<TransformInfo, re_tf::TransformFromToError>>,
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
        let (transform_forest, transform_cache) =
            caches.entry(|c: &mut TransformDatabaseStoreCache| {
                (
                    c.get_or_create_transform_forest(ctx.recording(), &ctx.current_query()),
                    c.read_lock_transform_cache(ctx.recording()),
                )
            });

        let frame_ids = transform_cache
            .frame_id_registry()
            .iter_frame_ids()
            .map(|(k, v)| (*k, v.clone()));

        Box::new(TransformTreeContextOncePerFrameResult {
            transform_forest,
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

        let results = query
            .iter_all_data_results()
            .map(|data_result| {
                let latest_at_query = ctx.current_query();

                let transform_frame_id_component =
                    archetypes::CoordinateFrame::descriptor_frame().component;

                let query_shadowed_components = false;
                latest_at_with_blueprint_resolved_data(
                    ctx,
                    None,
                    &latest_at_query,
                    data_result,
                    [transform_frame_id_component],
                    query_shadowed_components,
                )
            })
            .collect::<Vec<_>>();

        // Build a lookup table from entity paths to their transform frame id hashes.
        // Currently, we don't keep it around during the frame, but we may do so in the future.
        self.entity_transform_id_mapping =
            EntityTransformIdMapping::new(ctx, &results, query.space_origin);

        // Target frame - check for blueprint override first, otherwise use space origin's coordinate frame.
        self.target_frame = {
            let spatial_info_prop = ViewProperty::from_archetype::<
                blueprint::archetypes::SpatialInformation,
            >(
                ctx.blueprint_db(), ctx.blueprint_query(), ctx.view_id
            );

            let target_frame_component = spatial_info_prop
                .component_or_fallback::<TransformFrameId>(
                    ctx,
                    blueprint::archetypes::SpatialInformation::descriptor_target_frame().component,
                );

            match target_frame_component {
                Ok(target_frame) => TransformFrameIdHash::from_str(target_frame.as_str()),
                Err(err) => {
                    re_log::error_once!("Failed to query target frame: {err}");
                    self.transform_frame_id_for(query.space_origin.hash())
                }
            }
        };

        let latest_at_query = query.latest_at_query();

        // Add overrides to the transform frame id map so we can get back the id for errors.
        for results in results {
            let Some(frame) =
                results.get_mono(archetypes::CoordinateFrame::descriptor_frame().component)
            else {
                continue;
            };

            let frame_hash = TransformFrameIdHash::new(&frame);
            if !self.frame_id_mapping.contains_key(&frame_hash) {
                // As overrides are local to this view we need to clone the whole map to add new hashes.
                Arc::make_mut(&mut self.frame_id_mapping).insert(frame_hash, frame);
            }
        }

        let tree_transforms_per_frame = self
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

            let caches = ctx.viewer_ctx.store_context.caches;
            let transform_cache = caches.entry(|c: &mut TransformDatabaseStoreCache| {
                c.read_lock_transform_cache(ctx.recording())
            });
            let transforms = transform_cache.transforms_for_timeline(query.timeline);

            let latest_at_query = &query.latest_at_query();

            tree_transforms_per_frame
                .into_iter()
                .filter_map(|(transform_frame_id_hash, tree_transform)| {
                    let entity_paths_for_frame = self
                        .entity_transform_id_mapping
                        .transform_frame_id_to_entity_path
                        .get(&transform_frame_id_hash)?;
                    let transform_infos =
                        entity_paths_for_frame.iter().map(move |entity_path_hash| {
                            let transform_info = map_tree_transform_to_transform_info(
                                ctx,
                                &tree_transform,
                                transforms,
                                latest_at_query,
                                entity_path_hash,
                            );
                            (*entity_path_hash, transform_info)
                        });

                    Some(transform_infos)
                })
                .flatten()
                .collect()
        };

        self.target_frame_pinhole_root = self
            .transform_forest
            .root_from_frame(self.target_frame)
            .and_then(|info| {
                self.transform_forest
                    .pinhole_tree_root_info(info.root)
                    .map(|_pinhole_info| info.root)
            });
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn map_tree_transform_to_transform_info(
    ctx: &ViewContext<'_>,
    tree_transform: &Result<TreeTransform, re_tf::TransformFromToError>,
    transforms: &re_tf::CachedTransformsForTimeline,
    latest_at_query: &LatestAtQuery,
    entity_path_hash: &EntityPathHash,
) -> Result<TransformInfo, re_tf::TransformFromToError> {
    let tree_transform = tree_transform.as_ref().map_err(|err| err.clone())?;
    let poses = transforms
        .pose_transforms(*entity_path_hash)
        .map(|pose_transforms| {
            pose_transforms.latest_at_instance_poses(ctx.recording(), latest_at_query)
        })
        .unwrap_or_default();

    Ok(TransformInfo::new(tree_transform, poses))
}

impl TransformTreeContext {
    /// Returns a transform info describing how to get to the view's target frame for the given entity.
    #[inline]
    pub fn target_from_entity_path(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> Option<&Result<TransformInfo, re_tf::TransformFromToError>> {
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
    entity_path: &SmallVec1<[EntityPathHash; 1]>,
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
    let entity_path_hash = *entity_path.first();

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
    fn new(
        ctx: &ViewContext<'_>,
        results: &[HybridLatestAtResults<'_>],
        space_origin: &EntityPath,
    ) -> Self {
        // This is blueprint-dependent data and may also change over recording time,
        // making it non-trivial to cache.
        // That said, it rarely changes, so there's definitely an opportunity here for lazy updates!
        re_tracing::profile_function!();

        let mut mapping = Self::default();

        // Determine mapping for all _visible_ data results.
        for results in results {
            mapping.determine_frame_id_mapping_for(ctx, results);
        }

        // The origin entity may not be visible, make sure it's included.
        if !mapping
            .entity_path_to_transform_frame_id
            .contains_key(&space_origin.hash())
            && let Some(origin_data_result) = ctx
                .query_result
                .tree
                .lookup_result_by_path(space_origin.hash())
        {
            let latest_at_query = ctx.current_query();

            let transform_frame_id_component =
                archetypes::CoordinateFrame::descriptor_frame().component;

            let query_shadowed_components = false;
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_at_query,
                origin_data_result,
                [transform_frame_id_component],
                query_shadowed_components,
            );

            mapping.determine_frame_id_mapping_for(ctx, &results);
        }

        mapping
    }

    fn determine_frame_id_mapping_for(
        &mut self,
        ctx: &ViewContext<'_>,
        results: &HybridLatestAtResults<'_>,
    ) {
        let transform_frame_id_component =
            archetypes::CoordinateFrame::descriptor_frame().component;

        let frame_id = results
            .get_mono::<TransformFrameId>(transform_frame_id_component)
            .map_or_else(
                || {
                    let fallback =
                        TransformFrameIdHash::from_entity_path(&results.data_result.entity_path);
                    // Make sure this is the same as the fallback provider (which is a lot slower to run)
                    debug_assert_eq!(
                        TransformFrameIdHash::new(&typed_fallback_for::<TransformFrameId>(
                            &ctx.query_context(results.data_result, &results.query),
                            transform_frame_id_component
                        )),
                        fallback
                    );
                    fallback
                },
                |frame_id| TransformFrameIdHash::new(&frame_id),
            );

        let entity_path_hash = results.data_result.entity_path.hash();

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

#[cfg(test)]
mod tests {
    use re_log_types::{EntityPath, TimePoint};
    use re_test_context::TestContext;
    use re_test_viewport::TestContextExt as _;
    use re_tf::{TransformFrameId, TransformFrameIdHash};
    use re_types::archetypes::CoordinateFrame;
    use re_viewer_context::{
        BlueprintContext as _, RecommendedView, ViewClass as _, ViewClassExt as _,
        ViewContextSystem as _,
    };
    use re_viewport_blueprint::{ViewBlueprint, ViewContents, ViewProperty};

    use crate::SpatialView3D;
    use crate::contexts::TransformTreeContext;

    #[test]
    fn test_expected_target_frames() {
        let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();
        let class_id = SpatialView3D::identifier();

        test_context.log_entity("has_frame", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &CoordinateFrame::new("store_frame"))
        });

        // Different views with different expected targets.
        let view_id_root = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new(class_id, RecommendedView::root()))
        });
        let view_id_some_path = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new(
                class_id,
                RecommendedView::new_single_entity("some/path"),
            ))
        });
        let _view_id_some_path_overridden =
            test_context.setup_viewport_blueprint(|ctx, blueprint| {
                let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
                    class_id,
                    RecommendedView::new_single_entity("some/path"),
                ));
                ctx.save_blueprint_archetype(
                    ViewContents::override_path_for_entity(view_id, &"some/path".into()),
                    &CoordinateFrame::new("overridden_frame"),
                );
                view_id
            });
        let view_id_coordinate_frame = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new(
                class_id,
                RecommendedView::new_single_entity("has_frame"),
            ))
        });
        let view_id_coordinate_frame_overridden =
            test_context.setup_viewport_blueprint(|ctx, blueprint| {
                let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
                    class_id,
                    RecommendedView::new_single_entity("has_frame"),
                ));
                ctx.save_blueprint_archetype(
                    ViewContents::override_path_for_entity(view_id, &"has_frame".into()),
                    &CoordinateFrame::new("overridden_frame"),
                );
                view_id
            });
        let view_id_directly_set = test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let view_id =
                blueprint.add_view_at_root(ViewBlueprint::new(class_id, RecommendedView::root()));

            let property = ViewProperty::from_archetype::<
                re_types::blueprint::archetypes::SpatialInformation,
            >(ctx.blueprint_db(), ctx.blueprint_query(), view_id);
            property.save_blueprint_component(
                ctx,
                &re_types::blueprint::archetypes::SpatialInformation::descriptor_target_frame(),
                &TransformFrameId::from("directly_set_frame"),
            );

            view_id
        });

        test_context.run_in_egui_central_panel(|ctx, _ui| {
            for (view_id, expected_target) in [
                (
                    view_id_root,
                    TransformFrameId::from_entity_path(&EntityPath::root()),
                ),
                (
                    view_id_some_path,
                    TransformFrameId::from_entity_path(&EntityPath::from("some/path")),
                ),
                // TODO(RR-3076): this fails right now since if there's no data at all we don't create data results.
                // This will be fixed by either removing space_origin or by supporting override-only visualizations.
                // (
                //     _view_id_some_path_overridden,
                //     TransformFrameId::from("overridden_frame"),
                // ),
                (
                    view_id_coordinate_frame,
                    TransformFrameId::from("store_frame"),
                ),
                (
                    view_id_coordinate_frame_overridden,
                    TransformFrameId::from("overridden_frame"),
                ),
                (
                    view_id_directly_set,
                    TransformFrameId::from("directly_set_frame"),
                ),
            ] {
                let view_blueprint = ViewBlueprint::try_from_db(
                    view_id,
                    ctx.store_context.blueprint,
                    ctx.blueprint_query,
                )
                .expect("expected the view id to be known to the blueprint store");

                let view_class = SpatialView3D;
                let mut view_states = test_context.view_states.lock();
                let view_state = view_states.get_mut_or_create(view_id, &view_class);

                let view_ctx =
                    view_class.view_context(ctx, view_id, view_state, &view_blueprint.space_origin);
                let view_query = re_viewport::new_view_query(ctx, &view_blueprint);

                let mut tree_context = TransformTreeContext::default();
                let once_per_frame = TransformTreeContext::execute_once_per_frame(ctx);
                tree_context.execute(&view_ctx, &view_query, &once_per_frame);

                assert_eq!(
                    tree_context.target_frame(),
                    TransformFrameIdHash::new(&expected_target),
                    "View expected target frame {expected_target:?}, got {:?}",
                    tree_context.format_frame(tree_context.target_frame())
                );
            }
        });
    }
}
