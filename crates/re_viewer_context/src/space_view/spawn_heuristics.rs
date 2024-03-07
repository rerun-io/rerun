use re_log_types::{EntityPath, EntityPathFilter};

/// Properties of a space view that as recommended to be spawned by default via space view spawn heuristics.
#[derive(Hash)]
pub struct RecommendedSpaceView {
    pub root: EntityPath,
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
