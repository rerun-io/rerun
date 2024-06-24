use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;

use re_data_store::LatestAtQuery;
use re_log_types::{DataCell, DataRow, EntityPath, RowId};
use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{
    blueprint_timeline, ComponentUiTypes, QueryContext, SystemCommand, SystemCommandSender as _,
    ViewContext, ViewSystemIdentifier,
};
use re_viewport_blueprint::SpaceViewBlueprint;

pub fn defaults_ui(ctx: &ViewContext<'_>, space_view: &SpaceViewBlueprint, ui: &mut egui::Ui) {
    let db = ctx.viewer_ctx.blueprint_db();
    let query = ctx.viewer_ctx.blueprint_query;
    let resolver = Default::default();

    // Cleared components should act as unset, so we filter out everything that's empty,
    // even if they are listed in `all_components`.
    let active_defaults = ctx
        .blueprint_db()
        .store()
        .all_components(&blueprint_timeline(), &space_view.defaults_path)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| {
            db.query_caches()
                .latest_at(db.store(), query, &space_view.defaults_path, [*c])
                .components
                .get(c)
                .and_then(|data| data.resolved(&resolver).ok())
                .map_or(false, |data| !data.is_empty())
        })
        .collect::<BTreeSet<_>>();

    // It only makes sense to set defaults for components that are used by a system in the view.
    let mut component_to_vis: BTreeMap<ComponentName, ViewSystemIdentifier> = Default::default();

    // Accumulate the components across all visualizers and track which visualizer
    // each component came from so we can use it for fallbacks later.
    //
    // If two visualizers have the same component, the first one wins.
    // TODO(jleibs): We can do something fancier in the future such as presenting both
    // options once we have a motivating use-case.
    for (id, vis) in ctx.visualizer_collection.iter_with_identifiers() {
        for &component in vis.visualizer_query_info().queried.iter() {
            component_to_vis.entry(component).or_insert_with(|| id);
        }
    }

    add_new_default(
        ctx,
        query,
        ui,
        &component_to_vis,
        &active_defaults,
        &space_view.defaults_path,
    );

    let sorted_overrides = active_defaults.iter().sorted();

    let query_context = QueryContext {
        viewer_ctx: ctx.viewer_ctx,
        target_entity_path: &space_view.defaults_path,
        archetype_name: None,
        query,
        view_state: ctx.view_state,
        view_ctx: Some(ctx),
    };

    re_ui::list_item::list_item_scope(ui, "defaults", |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for component_name in sorted_overrides {
            let Some(visualizer_identifier) = component_to_vis.get(component_name) else {
                continue;
            };
            let Ok(visualizer) = ctx
                .visualizer_collection
                .get_by_identifier(*visualizer_identifier)
            else {
                re_log::warn!(
                    "Failed to resolve visualizer identifier {visualizer_identifier}, to a visualizer implementation"
                );
                continue;
            };

            // TODO(jleibs): We're already doing this query above as part of the filter. This is kind of silly to do it again.
            // Change the structure to avoid this.
            let component_data = db
                .query_caches()
                .latest_at(
                    db.store(),
                    query,
                    &space_view.defaults_path,
                    [*component_name],
                )
                .components
                .get(component_name)
                .cloned(); /* arc */

            if let Some(component_data) = component_data {
                let value_fn = |ui: &mut egui::Ui| {
                    ctx.viewer_ctx.component_ui_registry.singleline_edit_ui(
                        &query_context,
                        ui,
                        db,
                        &space_view.defaults_path,
                        *component_name,
                        &component_data,
                        visualizer.as_fallback_provider(),
                    );
                };

                ui.list_item()
                    .interactive(false)
                    .show_flat(
                        ui,
                        re_ui::list_item::PropertyContent::new(component_name.short_name())
                            .min_desired_width(150.0)
                            .action_button(&re_ui::icons::CLOSE, || {
                                ctx.save_empty_blueprint_component_by_name(
                                    &space_view.defaults_path,
                                    *component_name,
                                );
                            })
                            .value_fn(|ui, _| value_fn(ui)),
                    )
                    .on_hover_text(component_name.full_name());
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn add_new_default(
    ctx: &ViewContext<'_>,
    query: &LatestAtQuery,
    ui: &mut egui::Ui,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    active_overrides: &BTreeSet<ComponentName>,
    defaults_path: &EntityPath,
) {
    let remaining_components = component_to_vis
        .keys()
        .filter(|c| !active_overrides.contains(*c))
        .collect::<Vec<_>>();

    let enabled = !remaining_components.is_empty();

    ui.add_enabled_ui(enabled, |ui| {
        let mut opened = false;
        let menu = ui
            .menu_button("Add", |ui| {
                opened = true;
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                let query_context = QueryContext {
                    viewer_ctx: ctx.viewer_ctx,
                    target_entity_path: defaults_path,
                    archetype_name: None,
                    query,
                    view_state: ctx.view_state,
                    view_ctx: Some(ctx),
                };

                // Present the option to add new components for each component that doesn't
                // already have an active override.
                for (component, viz) in component_to_vis {
                    if active_overrides.contains(component) {
                        continue;
                    }

                    // If there is no registered editor, don't let the user create an override
                    // TODO(andreas): Can only handle single line editors right now.
                    if !ctx
                        .viewer_ctx
                        .component_ui_registry
                        .registered_ui_types(*component)
                        .contains(ComponentUiTypes::SingleLineEditor)
                    {
                        continue;
                    }

                    if ui.button(component.short_name()).clicked() {
                        // We are creating a new override. We need to decide what initial value to give it.
                        // - First see if there's an existing splat in the recording.
                        // - Next see if visualizer system wants to provide a value.
                        // - Finally, fall back on the default value from the component registry.

                        // TODO(jleibs): Is this the right place for fallbacks to come from?
                        let Some(mut initial_data) = ctx
                            .visualizer_collection
                            .get_by_identifier(*viz)
                            .ok()
                            .and_then(|sys| {
                                sys.fallback_for(&query_context, *component)
                                    .map(|fallback| DataCell::from_arrow(*component, fallback))
                                    .ok()
                            })
                        else {
                            re_log::warn!("Could not identify an initial value for: {}", component);
                            return;
                        };

                        initial_data.compute_size_bytes();

                        match DataRow::from_cells(
                            RowId::new(),
                            ctx.blueprint_timepoint_for_writes(),
                            defaults_path.clone(),
                            [initial_data],
                        ) {
                            Ok(row) => {
                                ctx.viewer_ctx.command_sender.send_system(
                                    SystemCommand::UpdateBlueprint(
                                        ctx.blueprint_db().store_id().clone(),
                                        vec![row],
                                    ),
                                );
                            }
                            Err(err) => {
                                re_log::warn!(
                                    "Failed to create DataRow for blueprint component: {}",
                                    err
                                );
                            }
                        }

                        ui.close_menu();
                    }
                }
            })
            .response
            .on_disabled_hover_text("No additional components available.");
        if !opened {
            menu.on_hover_text("Choose a component to specify an override value.".to_owned());
        }
    });
}
