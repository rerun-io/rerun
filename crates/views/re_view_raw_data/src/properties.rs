use itertools::Itertools as _;

use re_log_types::EntityPath;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::archetypes::RawDataRows;
use re_sdk_types::blueprint::components::ComponentIdentifier as BlueprintComponentIdentifier;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_ui::{UiExt as _, list_item};
use re_viewer_context::{ViewContext, ViewId, ViewerContext};
use re_viewport_blueprint::ViewProperty;

/// The components the view shows, without duplicates, or `None` when it should show every component.
pub fn data_rows(ctx: &ViewerContext<'_>, view_id: ViewId) -> Option<Vec<ComponentIdentifier>> {
    let data_rows = ViewProperty::from_archetype_for_view::<RawDataRows>(ctx, view_id)
        .component_array::<BlueprintComponentIdentifier>(
            RawDataRows::descriptor_data_rows().component,
        )
        .ok()
        .flatten()?;

    Some(
        data_rows
            .iter()
            .filter_map(|data_row| ComponentIdentifier::try_new(data_row.as_str()).ok())
            .unique()
            .collect(),
    )
}

/// Add `components` to the configured rows.
///
/// Does nothing when the view shows every component.
pub fn add_data_rows(ctx: &ViewContext<'_>, components: &[ComponentIdentifier]) {
    let Some(mut rows) = data_rows(ctx.viewer_ctx, ctx.view_id) else {
        return;
    };
    for component in components {
        if !rows.contains(component) {
            rows.push(*component);
        }
    }
    save_data_rows(ctx, Some(&rows));
}

/// Store the components to show, or clear them to show every component.
fn save_data_rows(ctx: &ViewContext<'_>, rows: Option<&[ComponentIdentifier]>) {
    let property = ViewProperty::from_archetype_for_view::<RawDataRows>(ctx, ctx.view_id);
    if let Some(rows) = rows {
        property.save_blueprint_component(
            ctx,
            &RawDataRows::descriptor_data_rows(),
            &rows
                .iter()
                .map(|component| BlueprintComponentIdentifier::from(component.as_str()))
                .collect::<Vec<_>>(),
        );
    } else {
        // Cleared implies showing all.
        property.clear_blueprint_component(ctx, RawDataRows::descriptor_data_rows());
    }
}

/// Edit the displayed entity using a single-entity path filter instead of the standard multi-entity filter.
pub fn entity_ui(ctx: &ViewContext<'_>, ui: &mut egui::Ui, displayed: Option<&EntityPath>) {
    let note = displayed
        .filter(|_| 1 < ctx.lookup_query_result(ctx.view_id).num_matching_entities)
        .map(|entity_path| {
            format!("View can only display 1 entity, currently displays {entity_path}")
        });
    let current = displayed.cloned().unwrap_or_else(EntityPath::root);

    ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Entity").value_fn(
        |ui, _| {
            ui.spacing_mut().text_edit_width = ui.available_width();

            let settled = re_data_ui::entity_path_edit_ui(
                ui,
                ctx.view_id.hash(),
                &current,
                &|| entities_with_data(ctx.viewer_ctx),
                &|entity_path| set_entity(ctx, entity_path),
            );

            if let Some(chosen) = settled {
                show_entity(ctx, &chosen, None);
            }
        },
    ));

    if let Some(note) = note {
        ui.error_label(note);
    }
}

/// Change the entity and replace its component checklist.
///
/// `rows` of `None` shows every component of the new entity.
pub fn show_entity(
    ctx: &ViewContext<'_>,
    entity_path: &EntityPath,
    rows: Option<&[ComponentIdentifier]>,
) {
    set_entity(ctx, entity_path);
    save_data_rows(ctx, rows);
}

/// Set a single-entity path filter.
fn set_entity(ctx: &ViewContext<'_>, entity_path: &EntityPath) {
    let Some(view) = re_viewport_blueprint::ViewBlueprint::try_from_db(
        ctx.view_id,
        ctx.blueprint_db(),
        ctx.blueprint_query(),
    ) else {
        return;
    };

    let filter = re_log_types::EntityPathFilter::single_entity_filter(entity_path);
    view.contents
        .set_entity_path_filter(ctx.viewer_ctx, filter.resolve_without_substitutions());
}

/// Every entity in the recording that has something to show.
fn entities_with_data(ctx: &ViewerContext<'_>) -> Vec<EntityPath> {
    let engine = ctx.recording_engine();
    let store = engine.store();

    store
        .all_entities()
        .into_iter()
        .filter(|entity_path| {
            store
                .all_components_on_timeline(Some(ctx.time_ctrl.timeline_name()), entity_path)
                .is_some_and(|components| !components.is_empty())
        })
        .sorted()
        .collect()
}

/// The "Components" section: a checklist of the entity's components, grouped by archetype.
pub fn components_section_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    entity_path: Option<&EntityPath>,
) {
    let Some(entity_path) = entity_path else {
        ui.add_space(6.0);
        ui.info_label("Pick an entity with the entity path filter above.");
        return;
    };

    let engine = ctx.recording_engine();
    let store = engine.store();

    let Some(available) = store
        .all_components_on_timeline(Some(ctx.viewer_ctx.time_ctrl.timeline_name()), entity_path)
    else {
        ui.info_label("This entity has no components on the active timeline.");
        return;
    };

    let by_archetype = re_data_ui::sorted_component_list_by_archetype_for_ui(
        ctx.viewer_ctx.reflection(),
        available
            .iter()
            .filter_map(|c| store.schema().entity_component_descriptor(entity_path, *c)),
    );

    let configured = data_rows(ctx.viewer_ctx, ctx.view_id);
    let alt = ui.ctx().format_modifiers(egui::Modifiers::ALT);
    let solo_hint = format!("{alt}+Click to show only this component, and again to show all");
    let last_visible_hint =
        format!("At least one component must be shown.\n{alt}+Click to show all");

    // An empty list reads back as unset, which shows everything, so the last tick box stays ticked.
    let num_visible = by_archetype
        .values()
        .flatten()
        .filter(|descr| is_shown(configured.as_deref(), descr.component))
        .count();

    let mut toggled = None;
    let mut is_first_group = true;

    for (archetype, components) in &by_archetype {
        if archetype.is_some() || 1 < by_archetype.len() {
            if !is_first_group {
                ui.add_space(4.0);
            }
            is_first_group = false;

            re_data_ui::archetype_label_list_item_ui(ui, archetype.as_ref());
        }

        for descr in components {
            let mut visible = is_shown(configured.as_deref(), descr.component);
            let is_last_visible = visible && num_visible == 1;

            ui.list_item_flat_noninteractive(list_item::CustomContent::new(|ui, _| {
                let solo = ui.input(|i| i.modifiers.alt);
                let response = ui
                    .add_enabled_ui(!is_last_visible || solo, |ui| {
                        ui.checkbox(&mut visible, descr.archetype_field_name())
                    })
                    .inner;

                if response.changed() {
                    if solo {
                        toggled = Some(Toggled::Only(descr.component));
                    } else if !is_last_visible {
                        toggled = Some(Toggled::One(descr.component, visible));
                    }
                }

                response
                    .on_disabled_hover_text(&last_visible_hint)
                    .on_hover_text(&solo_hint);
            }));
        }
    }

    if let Some(toggled) = toggled {
        let rows = toggled_rows(
            configured.as_deref(),
            by_archetype.values().flatten().map(|descr| descr.component),
            toggled,
        );
        save_data_rows(ctx, rows.as_deref());
    }
}

/// Is this component on show, given the rows configured for the view?
///
/// With nothing configured the view shows everything.
fn is_shown(configured: Option<&[ComponentIdentifier]>, component: ComponentIdentifier) -> bool {
    configured.is_none_or(|rows| rows.contains(&component))
}

/// What a click on a tick box asked for.
#[derive(Clone, Copy)]
enum Toggled {
    /// Show or hide this one component, leaving the rest as they were.
    One(ComponentIdentifier, bool),

    /// Solo this component, or show all if it is already the only visible component.
    Only(ComponentIdentifier),
}

/// The components on show after a toggle, or `None` to show every component.
///
/// The first toggle turns the view's implicit "show everything" into an explicit list. A component
/// logged after that is absent from the list and therefore hidden until the user ticks it.
fn toggled_rows(
    configured: Option<&[ComponentIdentifier]>,
    available: impl Iterator<Item = ComponentIdentifier> + Clone,
    toggled: Toggled,
) -> Option<Vec<ComponentIdentifier>> {
    let was_shown = |component| is_shown(configured, component);

    if let Toggled::Only(solo) = toggled
        && available
            .clone()
            .all(|component| was_shown(component) == (component == solo))
    {
        return None;
    }

    Some(
        available
            .filter(|&component| match toggled {
                Toggled::One(toggled, visible) if component == toggled => visible,
                Toggled::One(..) => was_shown(component),
                Toggled::Only(solo) => component == solo,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components(names: &[&'static str]) -> Vec<ComponentIdentifier> {
        names
            .iter()
            .copied()
            .map(ComponentIdentifier::from)
            .collect()
    }

    fn available() -> impl Iterator<Item = ComponentIdentifier> + Clone {
        components(&["a", "b", "c"]).into_iter()
    }

    #[test]
    fn solo_hides_the_rest() {
        let rows = toggled_rows(None, available(), Toggled::Only("b".into()));
        assert_eq!(rows, Some(components(&["b"])));
    }

    #[test]
    fn solo_on_the_only_visible_component_shows_all() {
        let rows = toggled_rows(
            Some(&components(&["b"])),
            available(),
            Toggled::Only("b".into()),
        );
        assert_eq!(rows, None);
    }

    #[test]
    fn toggle_leaves_unlisted_components_hidden() {
        let rows = toggled_rows(
            Some(&components(&["c"])),
            available(),
            Toggled::One("a".into(), true),
        );
        assert_eq!(
            rows,
            Some(components(&["a", "c"])),
            "`b` was not listed, so it stays hidden"
        );
    }
}
