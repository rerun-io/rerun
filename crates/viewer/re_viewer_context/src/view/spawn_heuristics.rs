use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, EntityPathFilter, EntityPathRule, EntityPathSubs};
use re_sdk_types::ViewClassIdentifier;

/// Maximum number of views of a single class that should be spawned in total.
///
/// This limit needs to be applied at the heuristics level (rather than just limiting the
/// number of recommended views per class) because we need to take existing views into account when
/// deciding whether to spawn new views.
/// For example, if max is 5 and there are already 3 views of this type, we should only spawn up to 2 more.
///
/// This limit applies on a per view class basis.
pub const MAX_VIEWS_SPAWNED: usize = 8;

/// Properties of a view that as recommended to be spawned by default via view spawn heuristics.
#[derive(Debug, Clone)]
pub struct RecommendedView {
    pub origin: EntityPath,
    pub query_filter: EntityPathFilter,
}

/// Heuristics for spawning views of a given class.
///
/// Provides information in order to decide whether to spawn a views, putting them in relationship to others and spawning them.
// TODO(andreas): Should `ViewClassLayoutPriority` be part of this struct?
#[derive(Debug)]
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

    /// Create new spawn heuristics preserving the input order.
    ///
    /// Use this when the input iterator is already sorted by priority
    /// (e.g., by match quality, datatype preference, etc.).
    // TODO(andreas): shouldn't we always do it like that? a bit confusing, but as of writing we rely on the sorting behavior of `new`.
    pub fn new_with_order_preserved(iter: impl IntoIterator<Item = RecommendedView>) -> Self {
        Self {
            recommended_views: iter.into_iter().collect(),
        }
    }

    #[inline]
    pub fn into_vec(self) -> Vec<RecommendedView> {
        self.recommended_views
    }
}

impl RecommendedView {
    #[inline]
    pub fn new_subtree(origin: impl Into<EntityPath>) -> Self {
        Self {
            origin: origin.into(),
            query_filter: EntityPathFilter::subtree_filter("$origin"),
        }
    }

    #[inline]
    pub fn new_single_entity(origin: impl Into<EntityPath>) -> Self {
        Self {
            origin: origin.into(),
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
    ) -> re_sdk_types::blueprint::components::ViewerRecommendationHash {
        let Self {
            origin,
            query_filter,
        } = self;

        Hash64::hash((origin, query_filter, class_id))
            .hash64()
            .into()
    }

    /// Crates new query filter rules for all given entities that would fit the current rule.
    pub fn exclude_entities(&mut self, excluded: &[EntityPath]) {
        let filter = self
            .query_filter
            .resolve_forgiving(&EntityPathSubs::new_with_origin(&self.origin));

        for e in excluded {
            if filter.matches(e) {
                self.query_filter.insert_rule(
                    re_log_types::RuleEffect::Exclude,
                    EntityPathRule::including_entity_subtree(e),
                );
            }
        }
    }
}
