use std::collections::BTreeMap;

use arrow::array::ArrayRef;
use itertools::Itertools as _;
use re_chunk::{ArchetypeName, Chunk, ComponentIdentifier, ComponentType, RowId};
use re_chunk_store::LatestAtQuery;
use re_data_ui::{DataUi as _, archetype_label_list_item_ui};
use re_log_types::EntityPath;
use re_types_core::ComponentDescriptor;
use re_types_core::reflection::ComponentDescriptorExt as _;
use re_ui::list_item::{LabelContent, ListItemContentButtonsExt as _};
use re_ui::{OnResponseExt as _, SyntaxHighlighting as _, UiExt as _};
use re_viewer_context::{
    ComponentUiTypes, QueryContext, SystemCommand, SystemCommandSender as _, UiLayout, ViewContext,
    VisualizerCollection, blueprint_timeline,
};
use re_viewport_blueprint::ViewBlueprint;

/// Entry that we can show in the defaults ui.
#[derive(Clone, Debug)]
struct DefaultOverrideEntry {
    component_type: ComponentType,
    component: ComponentIdentifier,
}

impl DefaultOverrideEntry {
    fn descriptor(&self, archetype_name: ArchetypeName) -> ComponentDescriptor {
        ComponentDescriptor {
            component_type: Some(self.component_type),
            component: self.component,
            archetype: Some(archetype_name),
        }
    }
}

pub fn view_components_defaults_section_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    view: &ViewBlueprint,
) {
    let db = ctx.viewer_ctx.blueprint_db();
    let query = ctx.blueprint_query();

    // TODO(andreas): Components in `active_defaults` should be sorted by field order within each archetype.
    // Right now, they're just sorted by descriptor, which is not the same.
    let active_defaults = active_defaults(ctx, view, db, query);
    let visualizers = ctx.new_visualizer_collection();
    let visualized_components_by_archetype =
        visualized_components_by_archetype(&visualizers, ctx.viewer_ctx.app_options());

    // If there is nothing set by the user and nothing to be possibly added, we skip the section
    // entirely.
    if active_defaults.is_empty() && visualized_components_by_archetype.is_empty() {
        return;
    }

    let components_to_show_in_add_menu =
        components_to_show_in_add_menu(ctx, &visualized_components_by_archetype, &active_defaults);
    let reason_we_cannot_add_more = components_to_show_in_add_menu.as_ref().err().cloned();

    let mut add_button_is_open = false;
    let mut add_button = ui
        .small_icon_button_widget(&re_ui::icons::ADD, "Add overrides…")
        .on_menu(|ui| {
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
        .on_hover_text("Add more component defaults");

    if let Some(reason) = reason_we_cannot_add_more {
        add_button = add_button.enabled(false).on_disabled_hover_text(reason);
    }

    let markdown = "# Component defaults\n
This section lists default values for components in the scope of the present view. The visualizers \
corresponding to this view's entities use these defaults when no per-entity store value or \
override is specified.\n
Click on the `+` button to add a new default value.";

    let body = |ui: &mut egui::Ui| {
        active_default_ui(ctx, ui, &active_defaults, view, query, db);
    };
    ui.section_collapsing_header("Component defaults")
        .with_button(add_button)
        .with_help_markdown(markdown)
        .show(ui, body);
}

fn active_default_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    active_defaults: &BTreeMap<ComponentIdentifier, ArrayRef>,
    view: &ViewBlueprint,
    query: &LatestAtQuery,
    db: &re_entity_db::EntityDb,
) {
    let query_context = QueryContext {
        view_ctx: ctx,
        target_entity_path: &view.defaults_path,
        instruction_id: None,
        archetype_name: None,
        query,
    };

    re_ui::list_item::list_item_scope(ui, "defaults", |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        if active_defaults.is_empty() {
            ui.list_item_flat_noninteractive(LabelContent::new("none").weak(true).italics(true));
        }

        let mut previous_archetype_name = None;

        for (component, default_value) in active_defaults {
            let Some(component_descr) = db
                .storage_engine()
                .store()
                .entity_component_descriptor(&view.defaults_path, *component)
            else {
                // Must mean the default wasn't active after all.
                continue;
            };

            if previous_archetype_name != component_descr.archetype {
                // `active_defaults` is sorted by descriptor which in turn sorts by archetype name,
                // so we can just check if the previous archetype name is different from the current one.
                if let Some(archetype_name) = component_descr.archetype {
                    archetype_label_list_item_ui(ui, &Some(archetype_name));
                    previous_archetype_name = Some(archetype_name);
                }
            }

            let value_fn = |ui: &mut egui::Ui| {
                let allow_multiline = false;
                ctx.viewer_ctx.component_ui_registry().edit_ui_raw(
                    &query_context,
                    ui,
                    db,
                    view.defaults_path.clone(),
                    &component_descr,
                    None, // No cache key.
                    default_value,
                    allow_multiline,
                );
            };

            let response = ui.list_item_flat_noninteractive(
                re_ui::list_item::PropertyContent::new(component_descr.archetype_field_name())
                    .min_desired_width(150.0)
                    .with_action_button(&re_ui::icons::CLOSE, "Clear blueprint component", || {
                        ctx.clear_blueprint_component(
                            view.defaults_path.clone(),
                            component_descr.clone(),
                        );
                    })
                    .value_fn(|ui, _| value_fn(ui)),
            );

            if let Some(component_type) = component_descr.component_type {
                response.on_hover_ui(|ui| {
                    component_type.data_ui_recording(
                        ctx.viewer_ctx,
                        ui,
                        re_viewer_context::UiLayout::Tooltip,
                    );
                });
            }
        }
    });
}

fn visualized_components_by_archetype(
    visualizers: &VisualizerCollection,
    app_options: &re_viewer_context::AppOptions,
) -> BTreeMap<ArchetypeName, Vec<DefaultOverrideEntry>> {
    let mut visualized_components_by_visualizer: BTreeMap<
        ArchetypeName,
        Vec<DefaultOverrideEntry>,
    > = Default::default();

    // It only makes sense to set defaults for components that are used by a system in the view.
    // Accumulate the components across all visualizers and track which visualizer
    // each component came from so we can use it for fallbacks later.
    for (id, vis) in visualizers.iter_with_identifiers() {
        for descr in vis.visualizer_query_info(app_options).queried.iter() {
            let (Some(archetype_name), Some(component_type)) =
                (descr.archetype, descr.component_type)
            else {
                // TODO(andreas): In theory this is perfectly valid: A visualizer may be interested in an untagged component!
                // Practically this never happens and we don't handle this in the ui here yet.
                re_log::warn_once!(
                    "Visualizer {} queried untagged component {}. It won't show in the defaults ui.",
                    id,
                    descr
                );
                continue;
            };

            visualized_components_by_visualizer
                .entry(archetype_name)
                .or_default()
                .push(DefaultOverrideEntry {
                    component_type,
                    component: descr.component,
                });
        }
    }

    visualized_components_by_visualizer
}

fn active_defaults(
    ctx: &ViewContext<'_>,
    view: &ViewBlueprint,
    db: &re_entity_db::EntityDb,
    query: &LatestAtQuery,
) -> BTreeMap<ComponentIdentifier, ArrayRef> {
    // Cleared components should act as unset, so we filter out everything that's empty,
    // even if they are listed in `all_components`.
    ctx.blueprint_db()
        .storage_engine()
        .store()
        .all_components_on_timeline(&blueprint_timeline(), &view.defaults_path)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|component| {
            let data = db
                .storage_engine()
                .cache()
                .latest_at(query, &view.defaults_path, [component])
                .component_batch_raw(component)?;
            (!data.is_empty()).then_some((component, data))
        })
        .collect()
}

fn components_to_show_in_add_menu(
    ctx: &ViewContext<'_>,
    visualized_components_by_archetype: &BTreeMap<ArchetypeName, Vec<DefaultOverrideEntry>>,
    active_defaults: &BTreeMap<ComponentIdentifier, ArrayRef>,
) -> Result<BTreeMap<ArchetypeName, Vec<DefaultOverrideEntry>>, String> {
    if visualized_components_by_archetype.is_empty() {
        return Err("No components to visualize".to_owned());
    }

    let mut components_to_show_in_add_menu = visualized_components_by_archetype.clone();

    {
        // Filter out all components that already have an active default.
        for components in components_to_show_in_add_menu.values_mut() {
            components.retain(|entry| !active_defaults.contains_key(&entry.component));
        }
        components_to_show_in_add_menu.retain(|_, components| !components.is_empty());

        if components_to_show_in_add_menu.is_empty() {
            return Err("All components already have active defaults".to_owned());
        }
    }
    {
        // Make sure we have editors. If we don't, explain to the user.
        let mut missing_editors = vec![];

        for (archetype_name, components) in &mut components_to_show_in_add_menu {
            components.retain(|entry| {
                // If there is no registered editor, don't let the user create an override
                // TODO(andreas): Can only handle single line editors right now.
                let types = ctx
                    .viewer_ctx
                    .component_ui_registry()
                    .registered_ui_types(entry.component_type);

                if types.contains(ComponentUiTypes::SingleLineEditor) {
                    true // show it
                } else {
                    missing_editors.push(entry.descriptor(*archetype_name));
                    false // don't show
                }
            });
        }
        components_to_show_in_add_menu.retain(|_, components| !components.is_empty());

        if components_to_show_in_add_menu.is_empty() {
            return Err(format!(
                "Rerun lacks edit UI for: {}",
                missing_editors.iter().map(|c| c.display_name()).join(", ")
            ));
        }
    }

    Ok(components_to_show_in_add_menu)
}

fn add_popup_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    defaults_path: &EntityPath,
    query: &LatestAtQuery,
    components_to_show_in_add_menu: BTreeMap<ArchetypeName, Vec<DefaultOverrideEntry>>,
) {
    let query_context = QueryContext {
        view_ctx: ctx,
        target_entity_path: defaults_path,
        instruction_id: None,
        archetype_name: None,
        query,
    };

    // Present the option to add new components for each component that doesn't
    // already have an active override.
    for (archetype_name, components) in components_to_show_in_add_menu {
        ui.menu_button(archetype_name.syntax_highlighted(ui.style()), |ui| {
            for entry in components {
                let descriptor = entry.descriptor(archetype_name);
                if ui
                    .button(descriptor.archetype_field_name())
                    .on_hover_ui(|ui| {
                        entry.component_type.data_ui_recording(
                            ctx.viewer_ctx,
                            ui,
                            UiLayout::Tooltip,
                        );
                    })
                    .clicked()
                {
                    add_new_default(ctx, defaults_path, &query_context, descriptor);
                    ui.close();
                }
            }
        });
    }
}

fn add_new_default(
    ctx: &ViewContext<'_>,
    defaults_path: &EntityPath,
    query_context: &QueryContext<'_>,
    component_descr: ComponentDescriptor,
) {
    // We are creating a new override. We need to decide what initial value to give it.
    // - First see if there's an existing splat in the recording.
    // - Next see if visualizer system wants to provide a value.
    // - Finally, fall back on the default value from the component registry.
    let initial_data = query_context
        .viewer_ctx()
        .component_fallback_registry
        .fallback_for(
            component_descr.component,
            component_descr.component_type,
            query_context,
        );

    match Chunk::builder(defaults_path.clone())
        .with_row(
            RowId::new(),
            ctx.blueprint_timepoint_for_writes(),
            [(component_descr, initial_data)],
        )
        .build()
    {
        Ok(chunk) => {
            ctx.viewer_ctx
                .command_sender()
                .send_system(SystemCommand::AppendToStore(
                    ctx.blueprint_db().store_id().clone(),
                    vec![chunk],
                ));
        }
        Err(err) => {
            re_log::warn!("Failed to create Chunk for blueprint component: {err}");
        }
    }
}
