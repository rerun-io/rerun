use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_query::query_archetype;
use re_types::Archetype;
use re_viewer_context::{external::re_entity_db::EntityTree, SpaceViewId};

pub fn entity_path_for_space_view_sub_archetype<T: Archetype>(
    space_view_id: SpaceViewId,
    _blueprint_entity_tree: &EntityTree,
) -> EntityPath {
    // TODO(andreas,jleibs):
    // We want to search the subtree for occurrences of the property archetype here.
    // Only if none is found we make up a new (standardized) path.
    // There's some nuances to figure out what happens when we find the archetype several times.
    // Also, we need to specify what it means to "find" the archetype (likely just matching the indicator?).
    let space_view_blueprint_path = space_view_id.as_entity_path();

    // Use short_name instead of full_name since full_name has dots and looks too much like an indicator component.
    space_view_blueprint_path.join(&EntityPath::from_single_string(T::name().short_name()))
}

pub fn query_space_view_sub_archetype<T: Archetype>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (Result<T, re_query::QueryError>, EntityPath) {
    let path = entity_path_for_space_view_sub_archetype::<T>(space_view_id, blueprint_db.tree());

    (
        query_archetype(blueprint_db.store(), query, &path).and_then(|arch| arch.to_archetype()),
        path,
    )
}

pub fn query_space_view_sub_archetype_or_default<T: Archetype + Default>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (T, EntityPath) {
    let (arch, path) = query_space_view_sub_archetype(space_view_id, blueprint_db, query);
    (arch.unwrap_or_default(), path)
}
