use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_log_types::{EntityPath, EntityPathHash};
use re_types::{archetypes, components::ImagePlaneDistance};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    DataResultTree, IdentifiedViewSystem, ViewContext, ViewContextSystem,
    ViewContextSystemStaticExecResult,
};

use crate::{caches::TransformDatabaseStoreCache, visualizers::CamerasVisualizer};

// TODO: docs

#[derive(Clone)]
pub struct TransformTreeContext {
    transform_infos:
        IntMap<EntityPathHash, Result<re_tf::TransformInfo, re_tf::TransformFromToError>>,
    reference_path: EntityPathHash,
}

impl Default for TransformTreeContext {
    fn default() -> Self {
        Self {
            transform_infos: Default::default(),
            reference_path: EntityPath::root().hash(),
        }
    }
}

impl IdentifiedViewSystem for TransformTreeContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

struct TransformTreeContextStaticExecResult {
    transform_tree: re_tf::TransformTree,
}

impl ViewContextSystem for TransformTreeContext {
    fn execute_static(
        ctx: &re_viewer_context::ViewerContext<'_>,
    ) -> ViewContextSystemStaticExecResult {
        let caches = ctx.store_context.caches;
        let transform_cache = caches.entry(|c: &mut TransformDatabaseStoreCache| {
            c.read_lock_transform_cache(ctx.recording())
        });

        let transform_tree =
            re_tf::TransformTree::new(ctx.recording(), &transform_cache, &ctx.current_query());

        Box::new(TransformTreeContextStaticExecResult { transform_tree })
    }

    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
        static_execution_result: &ViewContextSystemStaticExecResult,
    ) {
        self.reference_path = query.space_origin.hash();

        let transform_tree = &static_execution_result
            .downcast_ref::<TransformTreeContextStaticExecResult>()
            .expect("Unexpected static execution result type")
            .transform_tree;

        self.transform_infos = transform_tree
            .transform_from_to(
                self.reference_path,
                query.iter_all_entities().map(|e| e.hash()),
            )
            .collect();

        self.patch_image_planes(ctx);
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
        self.transform_infos.get(&entity_path)?.as_ref().ok()
    }

    #[inline]
    pub fn reference_path(&self) -> EntityPathHash {
        self.reference_path
    }

    /// Scale the transform info by the image plane distance of their parent pinhole.
    ///
    /// Doing this late-stage allows us to have image plane distance be scene dependent
    /// (both via heuristics & blueprint overrides)
    fn patch_image_planes(&mut self, ctx: &ViewContext<'_>) {
        let mut image_plane_distance_cache = IntMap::default();
        let data_result_tree = &ctx.query_result.tree;
        let latest_at_query = &ctx.current_query();

        for result in self.transform_infos.values_mut() {
            let Ok(transform_info) = result else {
                continue;
            };
            let Some(twod_in_threed_info) = &mut transform_info.twod_in_threed_info else {
                continue;
            };

            let parent_pinhole = twod_in_threed_info.parent_pinhole.hash(); // TODO: can we make parentpinhole a hash?
            let parent_pinhole_image_plane_distance = *image_plane_distance_cache
                .entry(parent_pinhole)
                .or_insert_with(|| {
                    lookup_image_plane_distance(
                        ctx,
                        data_result_tree,
                        parent_pinhole,
                        latest_at_query,
                    )
                });

            // Scale transforms according to the image plane distance.
            *transform_info = transform_info.right_multiply(
                glam::Affine3A::from_translation(glam::Vec3::new(
                    0.0,
                    0.0,
                    parent_pinhole_image_plane_distance - 1.0,
                )) * glam::Affine3A::from_scale(glam::Vec3::splat(
                    parent_pinhole_image_plane_distance,
                )),
            );
        }
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    data_result_tree: &DataResultTree,
    entity_path: EntityPathHash,
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
