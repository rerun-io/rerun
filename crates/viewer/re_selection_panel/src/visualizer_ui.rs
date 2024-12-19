use itertools::Itertools;

use re_chunk::{ComponentName, RowId, UnitChunkShared};
use re_data_ui::{sorted_component_list_for_ui, DataUi};
use re_entity_db::EntityDb;
use re_log_types::{ComponentPath, EntityPath};
use re_types::blueprint::components::VisualizerOverrides;
use re_types_core::external::arrow::array::ArrayRef;
use re_ui::{list_item, UiExt as _};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    DataResult, QueryContext, UiLayout, ViewClassExt as _, ViewContext, ViewSystemIdentifier,
    VisualizerSystem,
};
use re_viewport_blueprint::ViewBlueprint;

pub fn visualizer_ui(
    ctx: &ViewContext<'_>,
    view: &ViewBlueprint,
    entity_path: &EntityPath,
    ui: &mut egui::Ui,
) {
    let query_result = ctx.lookup_query_result(view.id);
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path(entity_path)
        .cloned()
    else {
        ui.error_label("Entity not found in view");
        return;
    };
    let active_visualizers: Vec<_> = data_result.visualizers.iter().sorted().copied().collect();
    let available_inactive_visualizers = available_inactive_visualizers(
        ctx,
        ctx.recording(),
        view,
        &data_result,
        &active_visualizers,
    );

    let button = list_item::ItemMenuButton::new(&re_ui::icons::ADD, |ui| {
        menu_add_new_visualizer(
            ctx,
            ui,
            &data_result,
            &active_visualizers,
            &available_inactive_visualizers,
        );
    })
    .enabled(!available_inactive_visualizers.is_empty())
    .hover_text("Add additional visualizers")
    .disabled_hover_text("No additional visualizers available");

    let markdown = "# Visualizers

This section lists the active visualizers for the selected entity. Visualizers use an entity's \
components to display it in the current view.

Each visualizer lists the components it uses and their values. The component values may come from \
a variety of sources and can be overridden in place.

The final component value is determined using the following priority order:
- **Override**: A value set from the UI and/or the blueprint. It has the highest precedence and is \
always used if set.
- **Store**: If any, the value logged to the data store for this entity, e.g. via the SDK's `log` \
function.
- **Default**: If set, the default value for this component in the current view, which can be set \
in the blueprint or in the UI by selecting the view.
- **Fallback**: A context-sensitive value that is used if no other value is available. It is \
specific to the visualizer and the current view type.";

    ui.section_collapsing_header("Visualizers")
        .button(button)
        .help_markdown(markdown)
        .show(ui, |ui| {
            visualizer_ui_impl(ctx, ui, &data_result, &active_visualizers);
        });
}

pub fn visualizer_ui_impl(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
) {
    let override_path = data_result.individual_override_path();

    let remove_visualizer_button = |ui: &mut egui::Ui, vis_name: ViewSystemIdentifier| {
        let response = ui.small_icon_button(&re_ui::icons::CLOSE);
        if response.clicked() {
            let component = VisualizerOverrides::from(
                active_visualizers
                    .iter()
                    .filter(|v| *v != &vis_name)
                    .map(|v| re_types_core::ArrowString::from(v.as_str()))
                    .collect::<Vec<_>>(),
            );

            ctx.save_blueprint_component(override_path, &component);
        }
        response
    };

    list_item::list_item_scope(ui, "visualizers", |ui| {
        if active_visualizers.is_empty() {
            ui.list_item_flat_noninteractive(
                list_item::LabelContent::new("none")
                    .weak(true)
                    .italics(true),
            );
        }

        for &visualizer_id in active_visualizers {
            let default_open = true;

            // List all components that the visualizer may consume.
            if let Ok(visualizer) = ctx.visualizer_collection.get_by_identifier(visualizer_id) {
                ui.list_item()
                    .interactive(false)
                    .show_hierarchical_with_children(
                        ui,
                        ui.make_persistent_id(visualizer_id),
                        default_open,
                        list_item::LabelContent::new(visualizer_id.as_str())
                            .min_desired_width(150.0)
                            .with_buttons(|ui| remove_visualizer_button(ui, visualizer_id))
                            .always_show_buttons(true),
                        |ui| visualizer_components(ctx, ui, data_result, visualizer),
                    );
            } else {
                ui.list_item_flat_noninteractive(
                    list_item::LabelContent::new(format!("{visualizer_id} (unknown visualizer)"))
                        .weak(true)
                        .min_desired_width(150.0)
                        .with_buttons(|ui| remove_visualizer_button(ui, visualizer_id))
                        .always_show_buttons(true),
                );
            }
        }
    });
}

/// Possible sources for a value in the component resolve stack.
///
/// Mostly for convenience and readability.
enum ValueSource {
    Override,
    Store,
    Default,
    FallbackOrPlaceholder,
}

fn visualizer_components(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    visualizer: &dyn VisualizerSystem,
) {
    // Helper for code below
    fn non_empty_component_batch_raw(
        unit: Option<&UnitChunkShared>,
        component_name: &ComponentName,
    ) -> Option<(Option<RowId>, ArrayRef)> {
        let unit = unit?;
        let batch = unit.component_batch_raw(component_name)?;
        if batch.is_empty() {
            None
        } else {
            Some((unit.row_id(), batch))
        }
    }

    let query_info = visualizer.visualizer_query_info();

    let store_query = ctx.current_query();
    let query_ctx = ctx.query_context(data_result, &store_query);

    // Query fully resolved data.
    let query_shadowed_defaults = true;
    let query_result = latest_at_with_blueprint_resolved_data(
        ctx,
        None, // TODO(andreas): Figure out how to deal with annotation context here.
        &store_query,
        data_result,
        query_info.queried.iter().copied(),
        query_shadowed_defaults,
    );

    // TODO(andreas): Should we show required components in a special way?
    for component_name in sorted_component_list_for_ui(query_info.queried.iter()) {
        if component_name.is_indicator_component() {
            continue;
        }

        // TODO(andreas): What about annotation context?

        // Query all the sources for our value.
        // (technically we only need to query those that are shown, but rolling this out makes things easier).
        let result_override = query_result.overrides.get(&component_name);
        let raw_override = non_empty_component_batch_raw(result_override, &component_name);

        let result_store = query_result.results.get(&component_name);
        let raw_store = non_empty_component_batch_raw(result_store, &component_name);

        let result_default = query_result.defaults.get(&component_name);
        let raw_default = non_empty_component_batch_raw(result_default, &component_name);

        let raw_fallback = visualizer
            .fallback_provider()
            .fallback_for(&query_ctx, component_name);

        // Determine where the final value comes from.
        // Putting this into an enum makes it easier to reason about the next steps.
        let (value_source, (current_value_row_id, raw_current_value)) =
            match (raw_override.clone(), raw_store.clone(), raw_default.clone()) {
                (Some(override_value), _, _) => (ValueSource::Override, override_value),
                (None, Some(store_value), _) => (ValueSource::Store, store_value),
                (None, None, Some(default_value)) => (ValueSource::Default, default_value),
                (None, None, None) => (
                    ValueSource::FallbackOrPlaceholder,
                    (None, raw_fallback.clone()),
                ),
            };

        let override_path = data_result.individual_override_path();

        let value_fn = |ui: &mut egui::Ui, _style| {
            // Edit ui can only handle a single value.
            let multiline = false;
            if raw_current_value.len() > 1
                // TODO(andreas): If component_ui_registry's `edit_ui_raw` wouldn't need db & query context (i.e. a query) we could use this directly here.
                || !ctx.viewer_ctx.component_ui_registry.try_show_edit_ui(
                    ctx.viewer_ctx,
                    ui,
                    raw_current_value.as_ref()                    ,
                    override_path,
                    component_name,
                    multiline,
                )
            {
                // TODO(andreas): Unfortunately, display ui needs db & query. (fix that!)
                // In fact some display UIs will struggle since they try to query additional data from the store.
                // so we have to figure out what store and path things come from.
                #[allow(clippy::unwrap_used)] // We checked earlier that these values are valid!
                let (query, db, entity_path, latest_at_unit) = match value_source {
                    ValueSource::Override => (
                        ctx.viewer_ctx.blueprint_query,
                        ctx.blueprint_db(),
                        override_path.clone(),
                        result_override.unwrap(),
                    ),
                    ValueSource::Store => (
                        &store_query,
                        ctx.recording(),
                        data_result.entity_path.clone(),
                        result_store.unwrap(),
                    ),
                    ValueSource::Default => (
                        ctx.viewer_ctx.blueprint_query,
                        ctx.blueprint_db(),
                        ViewBlueprint::defaults_path(ctx.view_id),
                        result_default.unwrap(),
                    ),
                    ValueSource::FallbackOrPlaceholder => {
                        // Fallback values are always single values, so we can directly go to the component ui.
                        // TODO(andreas): db & entity path don't make sense here.
                        ctx.viewer_ctx.component_ui_registry.ui_raw(
                            ctx.viewer_ctx,
                            ui,
                            UiLayout::List,
                            &store_query,
                            ctx.recording(),
                            &data_result.entity_path,
                            component_name,
                            current_value_row_id,
                            raw_current_value.as_ref(),
                        );
                        return;
                    }
                };

                re_data_ui::ComponentPathLatestAtResults {
                    component_path: ComponentPath::new(entity_path, component_name),
                    unit: latest_at_unit,
                }
                .data_ui(ctx.viewer_ctx, ui, UiLayout::List, query, db);
            }
        };

        let add_children = |ui: &mut egui::Ui| {
            // NOTE: each of the override/store/etc. UI elements may well resemble each other much,
            // e.g. be the same edit UI. We must ensure that we seed egui kd differently for each of
            // them to avoid id clashes.

            // Override (if available)
            if let Some((row_id, raw_override)) = raw_override.as_ref() {
                ui.push_id("override", |ui| {
                    editable_blueprint_component_list_item(
                        &query_ctx,
                        ui,
                        "Override",
                        override_path,
                        component_name,
                        *row_id,
                        raw_override.as_ref(),
                    )
                    .on_hover_text("Override value for this specific entity in the current view");
                });
            }

            // Store (if available)
            if let Some(unit) = result_store {
                ui.push_id("store", |ui| {
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("Store").value_fn(|ui, _style| {
                            re_data_ui::ComponentPathLatestAtResults {
                                component_path: ComponentPath::new(
                                    data_result.entity_path.clone(),
                                    component_name,
                                ),
                                unit,
                            }
                            .data_ui(
                                ctx.viewer_ctx,
                                ui,
                                UiLayout::List,
                                &store_query,
                                ctx.recording(),
                            );
                        }),
                    )
                    .on_hover_text("The value that was logged to the data store");
                });
            }

            // Default (if available)
            if let Some((row_id, raw_default)) = raw_default.as_ref() {
                ui.push_id("default", |ui| {
                    editable_blueprint_component_list_item(
                        &query_ctx,
                        ui,
                        "Default",
                        &ViewBlueprint::defaults_path(ctx.view_id),
                        component_name,
                        *row_id,
                        raw_default.as_ref(),
                    )
                    .on_hover_text(
                        "Default value for all components of this type is the current view",
                    );
                });
            }

            // Fallback (always there)
            {
                ui.push_id("fallback", |ui| {
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new("Fallback").value_fn(|ui, _| {
                            // TODO(andreas): db & entity path don't make sense here.
                            ctx.viewer_ctx.component_ui_registry.ui_raw(
                                ctx.viewer_ctx,
                                ui,
                                UiLayout::List,
                                &store_query,
                                ctx.recording(),
                                &data_result.entity_path,
                                component_name,
                                None,
                                raw_fallback.as_ref(),
                            );
                        }),
                    )
                    .on_hover_text(
                        "Context sensitive fallback value for this component type, used only if \
                    nothing else was specified. Unlike the other values, this may differ per \
                    visualizer.",
                    );
                });
            }
        };

        let default_open = false;
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(component_name),
                default_open,
                list_item::PropertyContent::new(component_name.short_name())
                    .value_fn(value_fn)
                    .show_only_when_collapsed(false)
                    .menu_button(&re_ui::icons::MORE, |ui: &mut egui::Ui| {
                        menu_more(
                            ctx,
                            ui,
                            component_name,
                            override_path,
                            &raw_override.clone().map(|(_, raw_override)| raw_override),
                            raw_default.clone().map(|(_, raw_override)| raw_override),
                            raw_fallback.clone(),
                            raw_current_value.clone(),
                        );
                    }),
                add_children,
            )
            .item_response
            .on_hover_ui(|ui| {
                component_name.data_ui_recording(ctx.viewer_ctx, ui, UiLayout::Tooltip);
            });
    }
}

fn editable_blueprint_component_list_item(
    query_ctx: &QueryContext<'_>,
    ui: &mut egui::Ui,
    name: &'static str,
    blueprint_path: &EntityPath,
    component: re_types::ComponentName,
    row_id: Option<RowId>,
    raw_override: &dyn arrow::array::Array,
) -> egui::Response {
    ui.list_item_flat_noninteractive(
        list_item::PropertyContent::new(name)
            .value_fn(|ui, _style| {
                let allow_multiline = false;
                query_ctx.viewer_ctx.component_ui_registry.edit_ui_raw(
                    query_ctx,
                    ui,
                    query_ctx.viewer_ctx.blueprint_db(),
                    blueprint_path,
                    component,
                    row_id,
                    raw_override,
                    allow_multiline,
                );
            })
            .action_button(&re_ui::icons::CLOSE, || {
                query_ctx
                    .viewer_ctx
                    .clear_blueprint_component_by_name(blueprint_path, component);
            }),
    )
}

/// "More" menu for a component line in the visualizer ui.
#[allow(clippy::too_many_arguments)]
fn menu_more(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_name: re_types::ComponentName,
    override_path: &EntityPath,
    raw_override: &Option<ArrayRef>,
    raw_default: Option<ArrayRef>,
    raw_fallback: arrow::array::ArrayRef,
    raw_current_value: arrow::array::ArrayRef,
) {
    if ui
        .add_enabled(raw_override.is_some(), egui::Button::new("Remove override"))
        .on_disabled_hover_text("There's no override active")
        .clicked()
    {
        ctx.clear_blueprint_component_by_name(override_path, component_name);
        ui.close_menu();
    }

    if ui
        .add_enabled(
            raw_default.is_some(),
            egui::Button::new("Set to view default value"),
        )
        .on_disabled_hover_text("There's no default component active")
        .clicked()
    {
        if let Some(raw_default) = raw_default {
            ctx.save_blueprint_array(override_path, component_name, raw_default);
        }
        ui.close_menu();
    }

    if ui.button("Set to fallback value").clicked() {
        ctx.save_blueprint_array(override_path, component_name, raw_fallback);
        ui.close_menu();
    }

    let override_differs_from_default = raw_override
        != &ctx
            .viewer_ctx
            .raw_latest_at_in_default_blueprint(override_path, component_name);
    if ui
        .add_enabled(
            override_differs_from_default,
            egui::Button::new("Reset override to default blueprint"),
        )
        .on_hover_text("Resets the override to what is specified in the default blueprint")
        .on_disabled_hover_text("Current override is the same as the override specified in the default blueprint (if any)")
        .clicked()
    {
        ctx.reset_blueprint_component_by_name(override_path, component_name);
        ui.close_menu();
    }

    if ui.button("Make default for current view").clicked() {
        ctx.save_blueprint_array(
            &ViewBlueprint::defaults_path(ctx.view_id),
            component_name,
            raw_current_value,
        );
        ui.close_menu();
    }
}

fn menu_add_new_visualizer(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
    inactive_visualizers: &[ViewSystemIdentifier],
) {
    let override_path = data_result.individual_override_path();

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    // Present an option to enable any visualizer that isn't already enabled.
    for viz in inactive_visualizers {
        if ui.button(viz.as_str()).clicked() {
            let component = VisualizerOverrides::from(
                active_visualizers
                    .iter()
                    .chain(std::iter::once(viz))
                    .map(|v| {
                        let arrow_str: re_types_core::ArrowString = v.as_str().into();
                        arrow_str
                    })
                    .collect::<Vec<_>>(),
            );

            ctx.save_blueprint_component(override_path, &component);

            ui.close_menu();
        }
    }
}

/// Lists all visualizers that are _not_ active for the given entity but could be.
fn available_inactive_visualizers(
    ctx: &ViewContext<'_>,
    entity_db: &EntityDb,
    view: &ViewBlueprint,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
) -> Vec<ViewSystemIdentifier> {
    // TODO(jleibs): This has already been computed for the View this frame. Maybe We
    // should do this earlier and store it with the View?
    let applicable_entities_per_visualizer = ctx
        .viewer_ctx
        .view_class_registry
        .applicable_entities_for_visualizer_systems(&entity_db.store_id());

    let visualizable_entities = view
        .class(ctx.viewer_ctx.view_class_registry)
        .determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &ctx.visualizer_collection,
            &view.space_origin,
        );

    visualizable_entities
        .iter()
        .filter(|&(vis, ents)| {
            ents.contains(&data_result.entity_path) && !active_visualizers.contains(vis)
        })
        .map(|(vis, _)| *vis)
        .sorted()
        .collect::<Vec<_>>()
}
