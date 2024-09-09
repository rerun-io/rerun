use re_log_types::{hash::Hash64, EntityPath, EntityPathFilter, EntityPathSubs};
use re_types::SpaceViewClassIdentifier;

/// Properties of a space view that as recommended to be spawned by default via space view spawn heuristics.
#[derive(Debug, Clone)]
pub struct RecommendedSpaceView {
    pub origin: EntityPath,
    pub query_filter: EntityPathFilter,
}

/// Heuristics for spawning space views of a given class.
///
/// Provides information in order to decide whether to spawn a space views, putting them in relationship to others and spawning them.
// TODO(andreas): allow bucketing decisions for 0-n buckets for recommended space views.
// TODO(andreas): Should `SpaceViewClassLayoutPriority` be part of this struct?
#[derive(Default)]
pub struct SpaceViewSpawnHeuristics {
    /// The recommended space views to spawn
    recommended_space_views: Vec<RecommendedSpaceView>,
}

impl SpaceViewSpawnHeuristics {
    #[inline]
    pub fn empty() -> Self {
        Self {
            recommended_space_views: Vec::new(),
        }
    }

    #[inline]
    pub fn root() -> Self {
        Self {
            recommended_space_views: vec![RecommendedSpaceView::root()],
        }
    }

    pub fn new(iter: impl IntoIterator<Item = RecommendedSpaceView>) -> Self {
        let mut recommended_space_views: Vec<RecommendedSpaceView> = iter.into_iter().collect();
        recommended_space_views.sort_by(|a, b| a.origin.cmp(&b.origin));
        Self {
            recommended_space_views,
        }
    }

    #[inline]
    pub fn into_vec(self) -> Vec<RecommendedSpaceView> {
        self.recommended_space_views
    }
}

impl RecommendedSpaceView {
    #[inline]
    pub fn new<'a>(origin: EntityPath, expressions: impl IntoIterator<Item = &'a str>) -> Self {
        let space_env = EntityPathSubs::new_with_origin(&origin);
        Self {
            origin,
            query_filter: EntityPathFilter::from_query_expressions_forgiving(
                expressions,
                &space_env,
            ),
        }
    }

    #[inline]
    pub fn new_subtree(origin: EntityPath) -> Self {
        Self::new(origin, std::iter::once("$origin/**"))
    }

    #[inline]
    pub fn new_single_entity(origin: EntityPath) -> Self {
        Self::new(origin, std::iter::once("$origin"))
    }

    #[inline]
    pub fn root() -> Self {
        Self::new_subtree(EntityPath::root())
    }

    /// Hash together with the Space View class id to the `ViewerRecommendationHash` component.
    ///
    /// Recommendations are usually tied to a specific Space View class.
    /// Therefore, to identify a recommendation for identification purposes, the class id should be included in the hash.
    pub fn recommendation_hash(
        &self,
        class_id: SpaceViewClassIdentifier,
    ) -> re_types::blueprint::components::ViewerRecommendationHash {
        let Self {
            origin,
            query_filter,
        } = self;

        Hash64::hash((origin, query_filter, class_id))
            .hash64()
            .into()
    }
}
