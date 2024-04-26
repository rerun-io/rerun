use re_data_store::LatestAtQuery;
use re_entity_db::{
    external::re_query::{LatestAtResults, PromiseResult, ToArchetype},
    EntityDb,
};
use re_log_types::EntityPath;
use re_types::Archetype;
use re_viewer_context::{external::re_entity_db::EntityTree, SpaceViewId, ViewerContext};

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

/// Return the archetype value for the given space view, or `None` if it doesn't exist.
pub fn space_view_sub_archetype<A: re_types::Archetype>(
    ctx: &re_viewer_context::ViewerContext<'_>,
    space_view_id: re_viewer_context::SpaceViewId,
) -> Option<A>
where
    LatestAtResults: ToArchetype<A>,
{
    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let path = entity_path_for_space_view_sub_archetype::<A>(space_view_id, blueprint_db.tree());
    blueprint_db
        .latest_at_archetype(&path, blueprint_query)
        .ok()
        .flatten()
        .map(|(_index, value)| value)
}

/// Returns `Ok(None)` if any of the required components are missing.
pub fn query_space_view_sub_archetype<A: Archetype>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (PromiseResult<Option<A>>, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let path = entity_path_for_space_view_sub_archetype::<A>(space_view_id, blueprint_db.tree());
    (
        blueprint_db
            .latest_at_archetype(&path, query)
            .map(|res| res.map(|(_, arch)| arch)),
        path,
    )
}

pub fn query_space_view_sub_archetype_or_default<A: Archetype + Default>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (A, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let (arch, path) = query_space_view_sub_archetype(space_view_id, blueprint_db, query);
    (arch.ok().flatten().unwrap_or_default(), path)
}

/// Read a single component of a blueprint archetype in a space view.
pub fn get_blueprint_component<A: re_types::Archetype, C: re_types::Component>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
) -> Option<C> {
    let blueprint_db = ctx.store_context.blueprint;
    let query = ctx.blueprint_query;
    let path = entity_path_for_space_view_sub_archetype::<A>(space_view_id, blueprint_db.tree());
    blueprint_db
        .latest_at_component::<C>(&path, query)
        .map(|x| x.value)
}

/// Edit a single component of a blueprint archetype in a space view.
pub fn edit_blueprint_component<A: re_types::Archetype, C: re_types::Component + PartialEq, R>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    edit_component: impl FnOnce(&mut Option<C>) -> R,
) -> R {
    let blueprint_db = ctx.store_context.blueprint;
    let query = ctx.blueprint_query;
    let path = entity_path_for_space_view_sub_archetype::<A>(space_view_id, blueprint_db.tree());
    let original = blueprint_db.latest_at_component::<C>(&path, query);
    let original: Option<C> = original.map(|x| x.value);

    let mut edited = original.clone();
    let ret = edit_component(&mut edited);

    if edited != original {
        ctx.save_blueprint_component(&path, &edited);
    }

    ret
}
