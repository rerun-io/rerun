use re_log_types::{EntityPath, EntityPathFilter, EntityPathSubs};

/// Properties of a space view that as recommended to be spawned by default via space view spawn heuristics.
#[derive(Hash, Debug, Clone)]
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
    pub recommended_space_views: Vec<RecommendedSpaceView>,
}

impl RecommendedSpaceView {
    #[inline]
    pub fn new<'a>(origin: EntityPath, expressions: impl IntoIterator<Item = &'a str>) -> Self {
        let space_env = EntityPathSubs::new_with_origin(&origin);
        Self {
            origin,
            query_filter: EntityPathFilter::from_query_expressions(expressions, &space_env),
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
}
