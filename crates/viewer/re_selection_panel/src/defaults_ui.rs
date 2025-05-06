use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools as _;

use re_chunk::{Chunk, RowId};
use re_chunk_store::LatestAtQuery;
use re_data_ui::{sorted_component_name_list_for_ui, DataUi as _};
use re_log_types::hash::Hash64;
use re_log_types::EntityPath;
use re_types::ComponentNameSet;
use re_types_core::{ComponentDescriptor, ComponentName};
use re_ui::{list_item::LabelContent, UiExt as _};
use re_viewer_context::{
    blueprint_timeline, ComponentUiTypes, QueryContext, SystemCommand, SystemCommandSender as _,
    UiLayout, ViewContext, ViewSystemIdentifier,
};
use re_viewport_blueprint::ViewBlueprint;

pub fn view_components_defaults_section_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    view: &ViewBlueprint,
) {
    let db = ctx.viewer_ctx.blueprint_db();
    let query = ctx.viewer_ctx.blueprint_query;

    let active_defaults = active_defaults(ctx, view, db, query);
    let component_to_vis = component_to_vis(ctx);

    // If there is nothing set by the user and nothing to be possibly added, we skip the section
    // entirely.
    if active_defaults.is_empty() && component_to_vis.is_empty() {
        return;
    }

    let components_to_show_in_add_menu =
        components_to_show_in_add_menu(ctx, &component_to_vis, &active_defaults);
    let reason_we_cannot_add_more = components_to_show_in_add_menu.as_ref().err().cloned();

    let mut add_button_is_open = false;
    let mut add_button = re_ui::list_item::ItemMenuButton::new(&re_ui::icons::ADD, |ui| {
        add_button_is_open = true;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add_popup_ui(
            ctx,
            ui,
            &view.defaults_path,
            query,
            components_to_show_in_add_menu.unwrap_or_default(),
        );
    })
    .hover_text("Add more component defaults");

    if let Some(reason) = reason_we_cannot_add_more {
        add_button = add_button.enabled(false).disabled_hover_text(reason);
    }

    let markdown = "# Component defaults\n
This section lists default values for components in the scope of the present view. The visualizers \
corresponding to this view's entities use these defaults when no per-entity store value or \
override is specified.\n
Click on the `+` button to add a new default value.";

    let body = |ui: &mut egui::Ui| {
        active_default_ui(
            ctx,
            ui,
            &active_defaults,
            &component_to_vis,
            view,
            query,
            db,
        );
    };
    ui.section_collapsing_header("Component defaults")
        .button(add_button)
        .help_markdown(markdown)
        .show(ui, body);
}

fn active_default_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    active_defaults: &ComponentNameSet,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    view: &ViewBlueprint,
    query: &LatestAtQuery,
    db: &re_entity_db::EntityDb,
) {
    let sorted_overrides = sorted_component_name_list_for_ui(active_defaults.iter());

    let query_context = QueryContext {
        viewer_ctx: ctx.viewer_ctx,
        target_entity_path: &view.defaults_path,
        archetype_name: None,
        query,
        view_state: ctx.view_state,
        view_ctx: Some(ctx),
    };

    re_ui::list_item::list_item_scope(ui, "defaults", |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        if sorted_overrides.is_empty() {
            ui.list_item_flat_noninteractive(LabelContent::new("none").weak(true).italics(true));
        }

        for component_name in sorted_overrides {
            let Some(visualizer_identifier) = component_to_vis.get(&component_name) else {
                continue;
            };
            let Ok(visualizer) = ctx
                .visualizer_collection
                .get_by_identifier(*visualizer_identifier)
            else {
                re_log::warn!(
                    "Failed to resolve visualizer identifier {visualizer_identifier}, to a \
                    visualizer implementation"
                );
                continue;
            };

            // TODO(jleibs): We're already doing this query above as part of the filter. This is kind of silly to do it again.
            // Change the structure to avoid this.
            let (row_id, component_array) = {
                let results = db.latest_at(query, &view.defaults_path, [component_name]);
                (
                    results.component_row_id(&component_name),
                    results.component_batch_raw(&component_name),
                )
            };

            if let Some(component_array) = component_array {
                let value_fn = |ui: &mut egui::Ui| {
                    ctx.viewer_ctx.component_ui_registry().singleline_edit_ui(
                        &query_context,
                        ui,
                        db,
                        &view.defaults_path,
                        component_name,
                        row_id.map(Hash64::hash),
                        Some(&*component_array),
                        visualizer.fallback_provider(),
                    );
                };

                ui.list_item_flat_noninteractive(
                    re_ui::list_item::PropertyContent::new(component_name.short_name())
                        .min_desired_width(150.0)
                        .action_button(&re_ui::icons::CLOSE, || {
                            ctx.clear_blueprint_component_by_name(
                                &view.defaults_path,
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
    view: &ViewBlueprint,
    db: &re_entity_db::EntityDb,
    query: &LatestAtQuery,
) -> ComponentNameSet {
    // Cleared components should act as unset, so we filter out everything that's empty,
    // even if they are listed in `all_components`.
    ctx.blueprint_db()
        .storage_engine()
        .store()
        .all_components_on_timeline(&blueprint_timeline(), &view.defaults_path)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| {
            db.storage_engine()
                .cache()
                .latest_at(query, &view.defaults_path, [c])
                .component_batch_raw_by_descr(c)
                .is_some_and(|data| !data.is_empty())
        })
        .map(|c| c.component_name)
        .collect::<BTreeSet<_>>()
}

fn components_to_show_in_add_menu(
    ctx: &ViewContext<'_>,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    active_defaults: &ComponentNameSet,
) -> Result<Vec<(ComponentName, ViewSystemIdentifier)>, String> {
    if component_to_vis.is_empty() {
        return Err("No components to visualize".to_owned());
    }

    let mut component_to_vis = component_to_vis
        .iter()
        .filter(|(component, _)| !active_defaults.contains(*component))
        .map(|(c, v)| (*c, *v))
        .collect::<Vec<_>>();

    if component_to_vis.is_empty() {
        return Err("All components already have active defaults".to_owned());
    }

    {
        // Make sure we have editors. If we don't, explain to the user.
        let mut missing_editors = vec![];

        component_to_vis.retain(|(component_name, _)| {
            // If there is no registered editor, don't let the user create an override
            // TODO(andreas): Can only handle single line editors right now.
            let types = ctx
                .viewer_ctx
                .component_ui_registry()
                .registered_ui_types(*component_name);

            if types.contains(ComponentUiTypes::SingleLineEditor) {
                true // show it
            } else {
                missing_editors.push(*component_name);
                false // don't show
            }
        });

        if component_to_vis.is_empty() {
            return Err(format!(
                "Rerun lacks edit UI for: {}",
                missing_editors.iter().map(|c| c.short_name()).join(", ")
            ));
        }
    }

    Ok(component_to_vis)
}

fn add_popup_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    defaults_path: &EntityPath,
    query: &LatestAtQuery,
    component_to_vis: Vec<(ComponentName, ViewSystemIdentifier)>,
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
    for (component_name, viz) in component_to_vis {
        #[allow(clippy::blocks_in_conditions)]
        if ui
            .button(component_name.short_name())
            .on_hover_ui(|ui| {
                component_name.data_ui_recording(ctx.viewer_ctx, ui, UiLayout::Tooltip);
            })
            .clicked()
        {
            // We are creating a new override. We need to decide what initial value to give it.
            // - First see if there's an existing splat in the recording.
            // - Next see if visualizer system wants to provide a value.
            // - Finally, fall back on the default value from the component registry.

            // TODO(jleibs): Is this the right place for fallbacks to come from?
            let Ok(visualizer) = ctx.visualizer_collection.get_by_identifier(viz) else {
                re_log::warn!("Could not find visualizer for: {}", viz);
                return;
            };
            let initial_data = visualizer
                .fallback_provider()
                .fallback_for(&query_context, component_name);

            match Chunk::builder(defaults_path.clone())
                .with_row(
                    RowId::new(),
                    ctx.blueprint_timepoint_for_writes(),
                    [(ComponentDescriptor::new(component_name), initial_data)],
                )
                .build()
            {
                Ok(chunk) => {
                    ctx.viewer_ctx
                        .command_sender()
                        .send_system(SystemCommand::UpdateBlueprint(
                            ctx.blueprint_db().store_id().clone(),
                            vec![chunk],
                        ));
                }
                Err(err) => {
                    re_log::warn!("Failed to create Chunk for blueprint component: {}", err);
                }
            }

            ui.close_menu();
        }
    }
}
