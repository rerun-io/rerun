use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools as _;
use re_data_store::LatestAtQuery;
use re_data_ui::{sorted_component_list_for_ui, DataUi as _};
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

    let active_defaults = active_defaults(ctx, space_view, db, query);
    let component_to_vis = component_to_vis(ctx);

    add_new_default(
        ctx,
        query,
        ui,
        &component_to_vis,
        &active_defaults,
        &space_view.defaults_path,
    );

    active_default_ui(
        ctx,
        ui,
        &active_defaults,
        &component_to_vis,
        space_view,
        query,
        db,
    );
}

fn active_default_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    active_defaults: &BTreeSet<ComponentName>,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    space_view: &SpaceViewBlueprint,
    query: &LatestAtQuery,
    db: &re_entity_db::EntityDb,
) {
    let sorted_overrides = sorted_component_list_for_ui(active_defaults.iter());

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
            let Some(visualizer_identifier) = component_to_vis.get(&component_name) else {
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
                    [component_name],
                )
                .components
                .get(&component_name)
                .cloned(); /* arc */

            if let Some(component_data) = component_data {
                let value_fn = |ui: &mut egui::Ui| {
                    ctx.viewer_ctx.component_ui_registry.singleline_edit_ui(
                        &query_context,
                        ui,
                        db,
                        &space_view.defaults_path,
                        component_name,
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
                                    component_name,
                                );
                            })
                            .value_fn(|ui, _| value_fn(ui)),
                    )
                    .on_hover_ui(|ui| {
                        component_name.data_ui_recording(
                            ctx.viewer_ctx,
                            ui,
                            re_viewer_context::UiLayout::Tooltip,
                        );
                    });
            }
        }
    });
}

fn component_to_vis(ctx: &ViewContext<'_>) -> BTreeMap<ComponentName, ViewSystemIdentifier> {
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
    component_to_vis
}

fn active_defaults(
    ctx: &ViewContext<'_>,
    space_view: &SpaceViewBlueprint,
    db: &re_entity_db::EntityDb,
    query: &LatestAtQuery,
) -> BTreeSet<ComponentName> {
    let resolver = Default::default();

    // Cleared components should act as unset, so we filter out everything that's empty,
    // even if they are listed in `all_components`.
    ctx.blueprint_db()
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
        .collect::<BTreeSet<_>>()
}

fn add_new_default(
    ctx: &ViewContext<'_>,
    query: &LatestAtQuery,
    ui: &mut egui::Ui,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    active_defaults: &BTreeSet<ComponentName>,
    defaults_path: &EntityPath,
) {
    let mut disabled_reason = None;
    if component_to_vis.is_empty() {
        disabled_reason = Some("No components to visualize".to_owned());
    }

    let mut component_to_vis = component_to_vis
        .iter()
        .filter(|(component, _)| !active_defaults.contains(*component))
        .collect::<Vec<_>>();

    if component_to_vis.is_empty() && disabled_reason.is_none() {
        disabled_reason = Some("All components already have active defaults".to_owned());
    }

    {
        // Make sure we have editors. If we don't, explain to the user.
        let mut missing_editors = vec![];

        component_to_vis.retain(|(component, _)| {
            let component = **component;

            // If there is no registered editor, don't let the user create an override
            // TODO(andreas): Can only handle single line editors right now.
            let types = ctx
                .viewer_ctx
                .component_ui_registry
                .registered_ui_types(component);

            if types.contains(ComponentUiTypes::SingleLineEditor) {
                true // show it
            } else {
                missing_editors.push(component);
                false // don't show
            }
        });

        if component_to_vis.is_empty() && disabled_reason.is_none() {
            disabled_reason = Some(format!(
                "Rerun lacks edit UI for: {}",
                missing_editors.iter().map(|c| c.short_name()).join(", ")
            ));
        }
    }

    let button_ui = |ui: &mut egui::Ui| -> egui::Response {
        let mut open = false;
        let menu = ui
            .menu_button("Add", |ui| {
                open = true;
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                add_popup_ui(ctx, ui, defaults_path, query, component_to_vis);
            })
            .response;
        if open {
            menu
        } else {
            menu.on_hover_text("Choose a component to specify an override value.".to_owned())
        }
    };

    let enabled = disabled_reason.is_none();
    let button_response = ui.add_enabled_ui(enabled, button_ui).inner;
    if let Some(disabled_reason) = disabled_reason {
        button_response.on_disabled_hover_text(disabled_reason);
    }
}

fn add_popup_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    defaults_path: &EntityPath,
    query: &LatestAtQuery,
    component_to_vis: Vec<(&ComponentName, &ViewSystemIdentifier)>,
) {
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
                    ctx.viewer_ctx
                        .command_sender
                        .send_system(SystemCommand::UpdateBlueprint(
                            ctx.blueprint_db().store_id().clone(),
                            vec![row],
                        ));
                }
                Err(err) => {
                    re_log::warn!("Failed to create DataRow for blueprint component: {}", err);
                }
            }

            ui.close_menu();
        }
    }
}
