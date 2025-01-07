use re_log_types::{hash::Hash64, EntityPath, EntityPathFilter};
use re_types::ViewClassIdentifier;

/// Properties of a view that as recommended to be spawned by default via view spawn heuristics.
#[derive(Debug, Clone)]
pub struct RecommendedView {
    pub origin: EntityPath,
    pub query_filter: EntityPathFilter,
}

/// Heuristics for spawning views of a given class.
///
/// Provides information in order to decide whether to spawn a views, putting them in relationship to others and spawning them.
// TODO(andreas): allow bucketing decisions for 0-n buckets for recommended views.
// TODO(andreas): Should `ViewClassLayoutPriority` be part of this struct?
#[derive(Default)]
pub struct ViewSpawnHeuristics {
    /// The recommended views to spawn
    recommended_views: Vec<RecommendedView>,
}

impl ViewSpawnHeuristics {
    #[inline]
    pub fn empty() -> Self {
        Self {
            recommended_views: Vec::new(),
        }
    }

    #[inline]
    pub fn root() -> Self {
        Self {
            recommended_views: vec![RecommendedView::root()],
        }
    }

    pub fn new(iter: impl IntoIterator<Item = RecommendedView>) -> Self {
        let mut recommended_views: Vec<RecommendedView> = iter.into_iter().collect();
        recommended_views.sort_by(|a, b| a.origin.cmp(&b.origin));
        Self { recommended_views }
    }

    #[inline]
    pub fn into_vec(self) -> Vec<RecommendedView> {
        self.recommended_views
    }
}

impl RecommendedView {
    #[inline]
    pub fn new_subtree(origin: EntityPath) -> Self {
        Self {
            origin,
            query_filter: EntityPathFilter::subtree_filter("$origin"),
        }
    }

    #[inline]
    pub fn new_single_entity(origin: EntityPath) -> Self {
        Self {
            origin,
            query_filter: EntityPathFilter::single_filter("$origin"),
        }
    }

    #[inline]
    pub fn root() -> Self {
        Self::new_subtree(EntityPath::root())
    }

    /// Hash together with the View class id to the `ViewerRecommendationHash` component.
    ///
    /// Recommendations are usually tied to a specific View class.
    /// Therefore, to identify a recommendation for identification purposes, the class id should be included in the hash.
    pub fn recommendation_hash(
        &self,
        class_id: ViewClassIdentifier,
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
