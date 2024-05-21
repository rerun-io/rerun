use re_data_store::LatestAtQuery;
use re_entity_db::{
    external::re_query::{LatestAtResults, PromiseResult, ToArchetype},
    EntityDb,
};
use re_log_types::EntityPath;
use re_types::Archetype;
use re_viewer_context::{
    external::{
        re_entity_db::EntityTree,
        re_ui::{self, list_item2},
    },
    SpaceViewId, ViewerContext,
};

pub fn entity_path_for_view_property<T: Archetype>(
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
pub fn view_property<A: re_types::Archetype>(
    ctx: &re_viewer_context::ViewerContext<'_>,
    space_view_id: re_viewer_context::SpaceViewId,
) -> Option<A>
where
    LatestAtResults: ToArchetype<A>,
{
    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());
    blueprint_db
        .latest_at_archetype(&path, blueprint_query)
        .ok()
        .flatten()
        .map(|(_index, value)| value)
}

/// Returns `Ok(None)` if any of the required components are missing.
pub fn query_view_property<A: Archetype>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (PromiseResult<Option<A>>, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());
    (
        blueprint_db
            .latest_at_archetype(&path, query)
            .map(|res| res.map(|(_, arch)| arch)),
        path,
    )
}

pub fn query_view_property_or_default<A: Archetype + Default>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (A, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let (arch, path) = query_view_property(space_view_id, blueprint_db, query);
    (arch.ok().flatten().unwrap_or_default(), path)
}

/// Read a single component of a blueprint archetype in a space view.
pub fn get_blueprint_component<A: re_types::Archetype, C: re_types::Component>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
) -> Option<C> {
    let blueprint_db = ctx.store_context.blueprint;
    let query = ctx.blueprint_query;
    let path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());
    blueprint_db
        .latest_at_component::<C>(&path, query)
        .map(|x| x.value)
}

/// Edit a single component of a blueprint archetype in a space view.
///
/// Set to `None` to reset the value to the value in the default blueprint, if any,
/// else will just store `None` (an empty component list) in the store.
pub fn edit_blueprint_component<A: re_types::Archetype, C: re_types::Component + PartialEq, R>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    edit_component: impl FnOnce(&mut Option<C>) -> R,
) -> R {
    let active_blueprint = ctx.store_context.blueprint;
    let active_path = entity_path_for_view_property::<A>(space_view_id, active_blueprint.tree());
    let original_value: Option<C> = active_blueprint
        .latest_at_component::<C>(&active_path, ctx.blueprint_query)
        .map(|x| x.value);

    let mut edited_value = original_value.clone();
    let ret = edit_component(&mut edited_value);

    if edited_value != original_value {
        if let Some(edited) = edited_value {
            ctx.save_blueprint_component(&active_path, &edited);
        } else {
            // Reset to the value in the default blueprint, if any.
            let default_value = ctx
                .store_context
                .default_blueprint
                .and_then(|default_blueprint| {
                    let default_path =
                        entity_path_for_view_property::<A>(space_view_id, default_blueprint.tree());
                    default_blueprint
                        .latest_at_component::<C>(&default_path, ctx.blueprint_query)
                        .map(|x| x.value)
                });
            ctx.save_blueprint_component(&active_path, &default_value);
        }
    }

    ret
}

/// Display the UI for editing all components of a blueprint archetype.
///
/// Note that this will show default values for components that are null.
pub fn view_property_ui<A: Archetype>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    ui: &mut egui::Ui,
) {
    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let blueprint_path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());

    let component_names = A::all_components();
    let component_results = blueprint_db.latest_at(
        blueprint_query,
        &blueprint_path,
        component_names.iter().copied(),
    );

    let sub_prop_ui = |re_ui: &re_ui::ReUi, ui: &mut egui::Ui| {
        for component_name in component_names.as_ref() {
            if component_name.is_indicator_component() {
                continue;
            }

            list_item2::ListItem::new(re_ui)
                .interactive(false)
                .show_flat(
                    ui,
                    // TODO(andreas): Note that we loose the archetype's field name here, instead we label the item with the component name.
                    list_item2::PropertyContent::new(component_name.short_name()).value_fn(
                        |_, ui, _| {
                            ctx.component_ui_registry.edit_ui(
                                ctx,
                                ui,
                                re_viewer_context::UiLayout::List,
                                blueprint_query,
                                blueprint_db,
                                &blueprint_path,
                                &blueprint_path,
                                component_results.get_or_empty(*component_name),
                                component_name,
                                &0.into(),
                            );
                        },
                    ),
                );
        }
    };

    list_item2::ListItem::new(ctx.re_ui)
        .interactive(false)
        .show_hierarchical_with_children(
            ui,
            A::name().full_name(),
            true,
            list_item2::LabelContent::new(A::name().short_name()),
            sub_prop_ui,
        );
}
