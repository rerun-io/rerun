use itertools::Itertools;

use re_data_ui::DataUi;
use re_entity_db::EntityDb;
use re_log_types::{DataCell, EntityPath};
use re_query::LatestAtComponentResults;
use re_space_view::latest_at_with_blueprint_resolved_data;
use re_types::external::arrow2;
use re_types_core::components::VisualizerOverrides;
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    DataResult, SpaceViewClassExt as _, UiLayout, ViewContext, ViewSystemIdentifier,
};
use re_viewport_blueprint::SpaceViewBlueprint;

pub fn visualizer_ui(
    ctx: &ViewContext<'_>,
    space_view: &SpaceViewBlueprint,
    entity_path: &EntityPath,
    ui: &mut egui::Ui,
) {
    let query_result = ctx.lookup_query_result(space_view.id);
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path(entity_path)
        .cloned()
    else {
        ui.error_label("Entity not found in view.");
        return;
    };
    let active_visualizers: Vec<_> = data_result.visualizers.iter().sorted().copied().collect();
    let available_inactive_visualizers = available_inactive_visualizers(
        ctx,
        ctx.recording(),
        space_view,
        &data_result,
        &active_visualizers,
    );

    ui.large_collapsing_header_with_button(
        "Visualizers",
        true,
        |ui| {
            visualizer_ui_impl(ctx, ui, &data_result, &active_visualizers);
        },
        re_ui::HeaderMenuButton::new(&re_ui::icons::ADD, |ui| {
            menu_add_new_visualizer(
                ctx,
                ui,
                &data_result,
                &active_visualizers,
                &available_inactive_visualizers,
            );
        })
        .with_enabled(!available_inactive_visualizers.is_empty())
        .with_hover_text("Add additional visualizers")
        .with_disabled_hover_text("No additional visualizers available"),
    );
}

pub fn visualizer_ui_impl(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
) {
    let Some(override_path) = data_result.individual_override_path() else {
        if cfg!(debug_assertions) {
            re_log::error!("No override path for entity: {}", data_result.entity_path);
        }
        return;
    };

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
        ui.spacing_mut().item_spacing.y = 0.0;

        for &visualizer_id in active_visualizers {
            let default_open = true;
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
                    |ui| visualizer_components(ctx, ui, data_result, visualizer_id),
                );
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
    visualizer_id: ViewSystemIdentifier,
) {
    // List all components that the visualizer may consume.
    let Ok(visualizer) = ctx.visualizer_collection.get_by_identifier(visualizer_id) else {
        re_log::warn!(
            "Failed to resolve visualizer identifier {visualizer_id}, to a visualizer implementation"
        );
        return;
    };

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
    for &component in query_info.queried.iter() {
        if component.is_indicator_component() {
            continue;
        }

        // TODO(andreas): What about annotation context?

        // Query all the sources for our value.
        // (technically we only need to query those that are shown, but rolling this out makes things easier).
        let result_override = query_result.overrides.get(component);
        let raw_override = result_override.and_then(|r| {
            r.try_raw(&query_result.resolver)
                .and_then(|v| (!v.is_empty()).then_some(v))
        });

        let result_store = query_result.results.get(component);
        let raw_store = result_store.and_then(|r| {
            r.try_raw(&query_result.resolver)
                .and_then(|v| (!v.is_empty()).then_some(v))
        });

        let result_default = query_result.defaults.get(component);
        let raw_default = result_default.and_then(|r| {
            r.try_raw(&query_result.resolver)
                .and_then(|v| (!v.is_empty()).then_some(v))
        });

        let raw_fallback = match visualizer.fallback_for(&query_ctx, component) {
            Ok(fallback) => fallback,
            Err(err) => {
                re_log::warn_once!("Failed to get fallback for component {component}: {err}");
                continue; // TODO(andreas): Don't give up on the entire component because of this. Show an error instead.
            }
        };

        // Determine where the final value comes from.
        // Putting this into an enum makes it easier to reason about the next steps.
        let (value_source, raw_current_value) = match (
            raw_override.as_ref(),
            raw_store.as_ref(),
            raw_default.as_ref(),
        ) {
            (Some(override_value), _, _) => (ValueSource::Override, override_value.as_ref()),
            (None, Some(store_value), _) => (ValueSource::Store, store_value.as_ref()),
            (None, None, Some(default_value)) => (ValueSource::Default, default_value.as_ref()),
            (None, None, None) => (ValueSource::FallbackOrPlaceholder, raw_fallback.as_ref()),
        };

        let Some(override_path) = data_result.individual_override_path() else {
            // This shouldn't the `DataResult` is valid.
            if cfg!(debug_assertions) {
                re_log::error!("No override path for entity: {}", data_result.entity_path);
            }
            return;
        };

        let value_fn = |ui: &mut egui::Ui, _style| {
            // Edit ui can only handle a single value.
            let multiline = false;
            if raw_current_value.len() > 1
                || !ctx.viewer_ctx.component_ui_registry.try_show_edit_ui(
                    ctx.viewer_ctx,
                    ui,
                    raw_current_value,
                    override_path,
                    component,
                    multiline,
                )
            {
                // TODO(andreas): Unfortunately, display ui needs db & query. (fix that!)
                // In fact some display UIs will struggle since they try to query additional data from the store.
                // so we have to figure out what store and path things come from.
                #[allow(clippy::unwrap_used)] // We checked earlier that these values are valid!
                let (query, db, entity_path, latest_at_results) = match value_source {
                    ValueSource::Override => (
                        ctx.viewer_ctx.blueprint_query,
                        ctx.blueprint_db(),
                        override_path,
                        result_override.unwrap(),
                    ),
                    ValueSource::Store => (
                        &store_query,
                        ctx.recording(),
                        &data_result.entity_path,
                        result_store.unwrap(),
                    ),
                    ValueSource::Default => (
                        ctx.viewer_ctx.blueprint_query,
                        ctx.blueprint_db(),
                        ctx.defaults_path,
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
                            component,
                            raw_current_value,
                        );
                        return;
                    }
                };

                re_data_ui::EntityLatestAtResults {
                    entity_path: entity_path.clone(),
                    results: latest_at_results,
                }
                .data_ui(ctx.viewer_ctx, ui, UiLayout::List, query, db);
            }
        };

        let add_children = |ui: &mut egui::Ui| {
            // Override (if available)
            if let (Some(result_override), Some(raw_override)) =
                (result_override, raw_override.as_ref())
            {
                editable_blueprint_component_list_item(
                    ctx,
                    ui,
                    "Override",
                    override_path,
                    component,
                    raw_override.as_ref(),
                    result_override,
                )
                .on_hover_text("Override value for this specific entity in the current view.");
            }
            // Store (if available)
            if let Some(result_store) = result_store {
                ui.list_item()
                    .interactive(false)
                    .show_flat(
                        ui,
                        list_item::PropertyContent::new("Store").value_fn(|ui, _style| {
                            re_data_ui::EntityLatestAtResults {
                                entity_path: data_result.entity_path.clone(),
                                results: result_store,
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
                    .on_hover_text("The value that was logged to the data store.");
            }
            // Default (if available)
            if let (Some(result_default), Some(raw_default)) =
                (result_default, raw_default.as_ref())
            {
                editable_blueprint_component_list_item(
                    ctx,
                    ui,
                    "Default",
                    ctx.defaults_path,
                    component,
                    raw_default.as_ref(),
                    result_default,
                )
                .on_hover_text("Default value for all component of this type is the current view.");
            }
            // Fallback (always there)
            {
                ui.list_item()
                    .interactive(false)
                    .show_flat(
                        ui,
                        list_item::PropertyContent::new("Fallback").value_fn(|ui, _| {
                            // TODO(andreas): db & entity path don't make sense here.
                            ctx.viewer_ctx.component_ui_registry.ui_raw(
                                ctx.viewer_ctx,
                                ui,
                                UiLayout::List,
                                &store_query,
                                ctx.recording(),
                                &data_result.entity_path,
                                component,
                                raw_fallback.as_ref(),
                            );
                        }),
                    )
                    .on_hover_text("Context sensitive fallback value for this component type, used only if nothing else was specified.
Unlike the other values, this may differ per visualizer.");
            }
        };

        let default_open = false;
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(component),
                default_open,
                list_item::PropertyContent::new(component.short_name())
                    .value_fn(value_fn)
                    .show_only_when_collapsed(false)
                    .menu_button(&re_ui::icons::MORE, |ui: &mut egui::Ui| {
                        menu_more(
                            ctx,
                            ui,
                            component,
                            override_path,
                            &raw_override,
                            &raw_default,
                            raw_fallback.as_ref(),
                            raw_current_value,
                        );
                    }),
                add_children,
            )
            .item_response
            .on_hover_text(component.full_name());
    }
}

fn editable_blueprint_component_list_item(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    name: &'static str,
    blueprint_path: &EntityPath,
    component: re_types::ComponentName,
    raw_override: &dyn arrow2::array::Array,
    result_override: &LatestAtComponentResults,
) -> egui::Response {
    ui.list_item().interactive(false).show_flat(
        ui,
        list_item::PropertyContent::new(name)
            .value_fn(|ui, _style| {
                let multiline = false;
                if raw_override.len() > 1
                    || !ctx.viewer_ctx.component_ui_registry.try_show_edit_ui(
                        ctx.viewer_ctx,
                        ui,
                        raw_override,
                        blueprint_path,
                        component,
                        multiline,
                    )
                {
                    re_data_ui::EntityLatestAtResults {
                        entity_path: blueprint_path.clone(),
                        results: result_override,
                    }
                    .data_ui(
                        ctx.viewer_ctx,
                        ui,
                        UiLayout::List,
                        ctx.viewer_ctx.blueprint_query,
                        ctx.blueprint_db(),
                    );
                }
            })
            .action_button(&re_ui::icons::CLOSE, || {
                ctx.save_empty_blueprint_component_by_name(blueprint_path, component);
            }),
    )
}

/// "More" menu for a component line in the visualizer ui.
#[allow(clippy::too_many_arguments)]
fn menu_more(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component: re_types::ComponentName,
    override_path: &EntityPath,
    raw_override: &Option<Box<dyn arrow2::array::Array>>,
    raw_default: &Option<Box<dyn arrow2::array::Array>>,
    raw_fallback: &dyn arrow2::array::Array,
    raw_current_value: &dyn arrow2::array::Array,
) {
    if ui
        .add_enabled(raw_override.is_some(), egui::Button::new("Remove override"))
        .on_disabled_hover_text("There's no override active")
        .clicked()
    {
        ctx.save_empty_blueprint_component_by_name(override_path, component);
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
        if let Some(raw_default) = raw_default.as_ref() {
            ctx.save_blueprint_data_cell(
                override_path,
                DataCell::from_arrow(component, raw_default.clone()),
            );
        }
        ui.close_menu();
    }

    if ui.button("Set to fallback value").clicked() {
        ctx.save_blueprint_data_cell(
            override_path,
            DataCell::from_arrow(component, raw_fallback.to_boxed()),
        );
        ui.close_menu();
    }

    if ui
        .button("Reset override")
        .on_hover_text("Resets the override to what is specified in the default blueprint.")
        .clicked()
    {
        ctx.reset_blueprint_component_by_name(override_path, component);
        ui.close_menu();
    }

    if ui.button("Make default for current view").clicked() {
        ctx.save_blueprint_data_cell(
            ctx.defaults_path,
            DataCell::from_arrow(component, raw_current_value.to_boxed()),
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
    // If we don't have an override_path we can't set up an initial override
    // this shouldn't happen if the `DataResult` is valid.
    let Some(override_path) = data_result.individual_override_path() else {
        if cfg!(debug_assertions) {
            re_log::error!("No override path for entity: {}", data_result.entity_path);
        }
        return;
    };

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
    space_view: &SpaceViewBlueprint,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
) -> Vec<ViewSystemIdentifier> {
    // TODO(jleibs): This has already been computed for the SpaceView this frame. Maybe We
    // should do this earlier and store it with the SpaceView?
    let applicable_entities_per_visualizer = ctx
        .viewer_ctx
        .space_view_class_registry
        .applicable_entities_for_visualizer_systems(entity_db.store_id());

    let visualizable_entities = space_view
        .class(ctx.viewer_ctx.space_view_class_registry)
        .determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &ctx.visualizer_collection,
            &space_view.space_origin,
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
