use arrow::datatypes::DataType;
use itertools::Itertools as _;
use re_chunk::{ComponentIdentifier, RowId};
use re_data_ui::{DataUi as _, sorted_component_list_by_archetype_for_ui};
use re_log_types::{ComponentPath, EntityPath};
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes::ActiveVisualizers;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_types_core::ComponentDescriptor;
use re_types_core::external::arrow::array::ArrayRef;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{OnResponseExt as _, UiExt as _, design_tokens_of_visuals, list_item};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    BlueprintContext as _, DataResult, PerVisualizer, QueryContext, UiLayout, ViewContext,
    ViewSystemIdentifier, VisualizerCollection, VisualizerExecutionErrorState,
    VisualizerInstruction, VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewBlueprint;

pub fn visualizer_ui(
    ctx: &ViewContext<'_>,
    view: &ViewBlueprint,
    visualizer_errors: &PerVisualizer<VisualizerExecutionErrorState>,
    entity_path: &EntityPath,
    ui: &mut egui::Ui,
) {
    let query_result = ctx.lookup_query_result(view.id);
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path(entity_path.hash())
        .cloned()
    else {
        ui.error_label("Entity not found in view");
        return;
    };
    let all_visualizers = ctx.new_visualizer_collection();
    let active_visualizers: Vec<_> = data_result
        .visualizer_instructions
        .iter()
        .cloned()
        .sorted_by_key(|instr| instr.visualizer_type)
        .collect();
    let available_visualizers = available_inactive_visualizers(ctx, &data_result);

    let button = ui
        .small_icon_button_widget(&re_ui::icons::ADD, "Add new visualizer…")
        .on_menu(|ui| {
            menu_add_new_visualizer(
                ctx,
                ui,
                &data_result,
                &active_visualizers,
                &available_visualizers,
            );
        })
        .enabled(!available_visualizers.is_empty())
        .on_hover_text("Add additional visualizers")
        .on_disabled_hover_text("No additional visualizers available");

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
        .with_button(button)
        .with_help_markdown(markdown)
        .show(ui, |ui| {
            visualizer_ui_impl(
                ctx,
                ui,
                &data_result,
                &active_visualizers,
                &all_visualizers,
                visualizer_errors,
            );
        });
}

pub fn visualizer_ui_impl(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[VisualizerInstruction],
    all_visualizers: &VisualizerCollection,
    visualizer_errors: &PerVisualizer<VisualizerExecutionErrorState>,
) {
    let override_base_path = data_result.override_base_path();

    let remove_visualizer_button = |ui: &mut egui::Ui, visualizer_id: &VisualizerInstructionId| {
        let response = ui.small_icon_button(&re_ui::icons::CLOSE, "Close");
        if response.clicked() {
            let active_visualizers = active_visualizers
                .iter()
                .filter(|v| &v.id != visualizer_id)
                .collect::<Vec<_>>();

            let archetype = ActiveVisualizers::new(active_visualizers.iter().map(|v| v.id.0));

            ctx.save_blueprint_archetype(override_base_path.clone(), &archetype);

            // If there's active visualizers, we also have to make sure that there's visualizer instructions, so time to manifest those.
            for visualizer_instruction in active_visualizers {
                visualizer_instruction.write_instruction_to_blueprint(ctx.viewer_ctx);
            }
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

        for (index, visualizer_instruction) in active_visualizers.iter().enumerate() {
            let visualizer_type = visualizer_instruction.visualizer_type;

            ui.push_id(index, |ui| {
                // List all components that the visualizer may consume.
                if let Ok(visualizer) = all_visualizers.get_by_type_identifier(visualizer_type) {
                    // Report whether this visualizer failed running.
                    let error_string = visualizer_errors
                        .get(&visualizer_type) // TODO(RR-3304): track errors per visualizer id, not per visualizer type.
                        .and_then(|error_state| {
                            error_state.error_string_for(&data_result.entity_path)
                        });

                    ui.list_item()
                        .with_y_offset(1.0)
                        .with_height(20.0)
                        .interactive(false)
                        .show_flat(
                            ui,
                            list_item::LabelContent::new(
                                egui::RichText::new(format!("{visualizer_type}"))
                                    .size(10.0)
                                    .color(
                                        design_tokens_of_visuals(ui.visuals())
                                            .list_item_strong_text,
                                    ),
                            )
                            .min_desired_width(150.0)
                            .with_buttons(|ui| {
                                remove_visualizer_button(ui, &visualizer_instruction.id);
                            })
                            .with_always_show_buttons(true),
                        );

                    if let Some(error_string) = error_string {
                        ui.error_label(error_string);
                    }

                    visualizer_components(ctx, ui, data_result, visualizer, visualizer_instruction);
                } else {
                    ui.list_item_flat_noninteractive(
                        list_item::LabelContent::new(format!(
                            "{visualizer_type} (unknown visualizer type)"
                        ))
                        .weak(true)
                        .min_desired_width(150.0)
                        .with_buttons(|ui| {
                            remove_visualizer_button(ui, &visualizer_instruction.id);
                        })
                        .with_always_show_buttons(true),
                    );
                }
            });
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
    instruction: &VisualizerInstruction,
) {
    let query_info = visualizer.visualizer_query_info(ctx.viewer_ctx.app_options());

    let store_query = ctx.current_query();
    let query_ctx = ctx.query_context(data_result, &store_query);

    // Query fully resolved data.
    let query_shadowed_defaults = true;
    let query_result = latest_at_with_blueprint_resolved_data(
        ctx,
        None, // TODO(andreas): Figure out how to deal with annotation context here.
        &store_query,
        data_result,
        query_info.queried_components(),
        query_shadowed_defaults,
        Some(instruction),
    );

    // Query all components of the entity so we can show them in the source component mapping UI.
    let entity_components_with_datatype = {
        let components = ctx
            .viewer_ctx
            .recording_engine()
            .store()
            .all_components_for_entity_sorted(&data_result.entity_path)
            .unwrap_or_default();
        components
            .into_iter()
            .filter_map(|component_id| {
                let component_type = ctx
                    .viewer_ctx
                    .recording_engine()
                    .store()
                    .lookup_component_type(&data_result.entity_path, component_id);
                component_type.map(|(_, arrow_datatype)| (component_id, arrow_datatype))
            })
            .collect::<Vec<_>>()
    };

    // TODO(andreas): Should we show required components in a special way?
    for unmapped_component_descr in sorted_component_list_by_archetype_for_ui(
        ctx.viewer_ctx.reflection(),
        query_info.queried.iter().cloned(),
    )
    .values()
    .flatten()
    {
        // TODO(andreas): What about annotation context?

        let target_component = unmapped_component_descr.component;

        // Have to apply component mappings if there are any for this component.
        let mapped_component = instruction
            .component_mappings
            .get(&unmapped_component_descr.component)
            .and_then(|mapping| match mapping {
                re_viewer_context::VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector: _, // TODO(RR-3308): implement selector logic
                } => Some(*source_component),

                re_viewer_context::VisualizerComponentSource::Override
                | re_viewer_context::VisualizerComponentSource::Default
                | re_viewer_context::VisualizerComponentSource::Fallback => {
                    // TODO(RR-3338): Implement ui for other types.
                    None
                }
            })
            .unwrap_or(unmapped_component_descr.component);

        // Query all the sources for our value.
        // (technically we only need to query those that are shown, but rolling this out makes things easier).
        let result_override = query_result.overrides.get(target_component);
        let raw_override =
            result_override.and_then(|c| c.non_empty_component_batch_raw(target_component));

        let result_store = query_result.results.get(mapped_component);
        let raw_store =
            result_store.and_then(|c| c.non_empty_component_batch_raw(mapped_component));

        let result_default = query_result.defaults.get(target_component);
        let raw_default =
            result_default.and_then(|c| c.non_empty_component_batch_raw(target_component));

        // If we don't have a component type, we don't have a way to retrieve a fallback. Therefore, we return a `NullArray` as a dummy.
        let raw_fallback = query_ctx
            .viewer_ctx()
            .component_fallback_registry
            .fallback_for(
                target_component,
                unmapped_component_descr.component_type,
                &query_ctx,
            );

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

        let override_path = &instruction.override_path;

        let value_fn = |ui: &mut egui::Ui, _style| {
            // Edit ui can only handle a single value.
            let multiline = false;
            if raw_current_value.len() > 1
                // TODO(andreas): If component_ui_registry's `edit_ui_raw` wouldn't need db & query context (i.e. a query) we could use this directly here.
                || !ctx.viewer_ctx.component_ui_registry().try_show_edit_ui(
                    ctx.viewer_ctx,
                    ui,
                    re_viewer_context::EditTarget {
                        store_id: ctx.viewer_ctx.store_context.blueprint.store_id().clone(),
                        timepoint: ctx.viewer_ctx.store_context.blueprint_timepoint_for_writes(),
                        entity_path: override_path.clone(),
                    },
                    raw_current_value.as_ref(),
                    unmapped_component_descr.clone(),
                    multiline,
                )
            {
                // TODO(andreas): Unfortunately, display ui needs db & query. (fix that!)
                // In fact some display UIs will struggle since they try to query additional data from the store.
                // so we have to figure out what store and path things come from.
                let (query, db, component_path_latest_at) = match value_source {
                    ValueSource::Override => (
                        ctx.blueprint_query(),
                        ctx.blueprint_db(),
                        re_data_ui::ComponentPathLatestAtResults {
                            component_path: ComponentPath::new(
                                override_path.clone(),
                                target_component,
                            ),
                            unit: result_override.expect("This value was validated earlier."),
                        },
                    ),
                    ValueSource::Store => (
                        &store_query,
                        ctx.recording(),
                        re_data_ui::ComponentPathLatestAtResults {
                            component_path: ComponentPath::new(
                                data_result.entity_path.clone(),
                                mapped_component,
                            ),
                            unit: result_store.expect("This value was validated earlier."),
                        },
                    ),
                    ValueSource::Default => (
                        ctx.blueprint_query(),
                        ctx.blueprint_db(),
                        re_data_ui::ComponentPathLatestAtResults {
                            component_path: ComponentPath::new(
                                ViewBlueprint::defaults_path(ctx.view_id),
                                target_component,
                            ),
                            unit: result_default.expect("This value was validated earlier."),
                        },
                    ),
                    ValueSource::FallbackOrPlaceholder => {
                        // Fallback values are always single values, so we can directly go to the component ui.
                        // TODO(andreas): db & entity path don't make sense here.
                        ctx.viewer_ctx.component_ui_registry().component_ui_raw(
                            ctx.viewer_ctx,
                            ui,
                            UiLayout::List,
                            &store_query,
                            ctx.recording(),
                            &data_result.entity_path,
                            unmapped_component_descr,
                            current_value_row_id,
                            raw_current_value.as_ref(),
                        );
                        return;
                    }
                };

                component_path_latest_at.data_ui(ctx.viewer_ctx, ui, UiLayout::List, query, db);
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
                        override_path.clone(),
                        unmapped_component_descr,
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
                                    unmapped_component_descr.component,
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
                        ViewBlueprint::defaults_path(ctx.view_id),
                        unmapped_component_descr,
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
                            ctx.viewer_ctx.component_ui_registry().component_ui_raw(
                                ctx.viewer_ctx,
                                ui,
                                UiLayout::List,
                                &store_query,
                                ctx.recording(),
                                &data_result.entity_path,
                                unmapped_component_descr,
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

            // Source component (if available).
            // TODO(RR-3338): Implement a new source componentselector UI.
            source_component_ui(
                ctx,
                ui,
                &entity_components_with_datatype,
                unmapped_component_descr,
                instruction,
                &query_info,
            );
        };

        let default_open = false;
        let response = ui
            .list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(target_component),
                default_open,
                list_item::PropertyContent::new(
                    // We're in the context of a visualizer, so we don't have to print the archetype name
                    // since usually archetypes match 1:1 with visualizers.
                    unmapped_component_descr.archetype_field_name(),
                )
                .value_fn(value_fn)
                .show_only_when_collapsed(false)
                .with_menu_button(&re_ui::icons::MORE, "More options", |ui: &mut egui::Ui| {
                    menu_more(
                        ctx,
                        ui,
                        unmapped_component_descr.clone(),
                        override_path,
                        &raw_override.clone().map(|(_, raw_override)| raw_override),
                        raw_default.clone().map(|(_, raw_override)| raw_override),
                        raw_fallback.clone(),
                        raw_current_value.clone(),
                    );
                })
                // TODO(emilk/egui#7531): Ideally we would hide the button unless hovered, but this
                // currently breaks the menu.
                .with_always_show_buttons(true),
                add_children,
            )
            .item_response;

        if let Some(component_type) = unmapped_component_descr.component_type {
            response.on_hover_ui(|ui| {
                // TODO(andreas): Add data ui for component descr?
                component_type.data_ui_recording(ctx.viewer_ctx, ui, UiLayout::Tooltip);
            });
        }
    }
}

fn source_component_ui(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    component_descr: &ComponentDescriptor,
    instruction: &VisualizerInstruction,
    query_info: &VisualizerQueryInfo,
) {
    if !ctx.viewer_ctx.app_options().experimental.component_mapping {
        return;
    }

    let Some(target_component_type) = &component_descr.component_type else {
        return;
    };

    let reflection = ctx.viewer_ctx.reflection();

    let is_required_component = if let Some(component_archetype) = component_descr.archetype
        && let Some(archetype_reflection) = reflection.archetypes.get(&component_archetype)
        && archetype_reflection
            .required_fields()
            .any(|field| field.component(component_archetype) == component_descr.component)
    {
        true
    } else {
        false
    };

    // Collect suitable source components with the same datatype as the target component.

    // TODO(andreas): Right now we are _more_ flexible for required components, because there we also support
    // casting in some special cases. Eventually this should always be the case, leaving us always with a list of valid physical types that we filter on.
    let allowed_physical_types = if is_required_component
        && let re_viewer_context::RequiredComponents::AnyPhysicalDatatype {
            semantic_type: _,
            physical_types,
        } = &query_info.required
    {
        physical_types.clone()
    } else {
        // Get arrow datatype of the target component.
        let Some(target_component_reflection) = reflection.components.get(target_component_type)
        else {
            return;
        };
        std::iter::once(target_component_reflection.datatype.clone()).collect()
    };

    let all_source_options = entity_components_with_datatype
        .iter()
        .filter(|entity_component| allowed_physical_types.contains(&entity_component.1))
        .map(|entity_component| entity_component.0.as_str())
        .collect::<Vec<_>>();
    if all_source_options.is_empty() {
        return;
    }

    ui.push_id("source_component", |ui| {
        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Source component").value_fn(|ui, _| {
                // Get the current source component from the component mapping.
                let current = instruction
                    .component_mappings
                    .get(&component_descr.component)
                    .and_then(|mapping| match mapping {
                        re_viewer_context::VisualizerComponentSource::SourceComponent {
                            source_component,
                            selector: _, // TODO(RR-3308): implement selector logic
                        } => Some(source_component.as_str()),

                        re_viewer_context::VisualizerComponentSource::Override
                        | re_viewer_context::VisualizerComponentSource::Default
                        | re_viewer_context::VisualizerComponentSource::Fallback => {
                            // TODO(RR-3338): Implement ui for other types.
                            None
                        }
                    })
                    .unwrap_or("");

                egui::ComboBox::new("source_component_combo_box", "")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for option in std::iter::once("").chain(all_source_options.into_iter()) {
                            if ui.button(option).clicked() {
                                save_component_mapping(
                                    ctx,
                                    instruction,
                                    option.into(),
                                    component_descr.component,
                                );
                            }
                        }
                    });
            }),
        );
    });
}

fn save_component_mapping(
    ctx: &ViewContext<'_>,
    instruction: &VisualizerInstruction,
    source_component: ComponentIdentifier,
    target: ComponentIdentifier,
) {
    let mut updated_instruction = instruction.clone();

    // Set or override the mapping
    match updated_instruction.component_mappings.entry(target) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            *entry.get_mut() = re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector: String::new(), // TODO(RR-3308): implement selector logic
            };
        }

        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(
                re_viewer_context::VisualizerComponentSource::SourceComponent {
                    source_component,
                    selector: String::new(), // TODO(RR-3308): implement selector logic
                },
            );
        }
    }

    // TODO(andreas): Don't write the type if it hasn't changed
    updated_instruction.write_instruction_to_blueprint(ctx.viewer_ctx);
}

fn editable_blueprint_component_list_item(
    query_ctx: &QueryContext<'_>,
    ui: &mut egui::Ui,
    name: &'static str,
    blueprint_path: EntityPath,
    component_descr: &ComponentDescriptor,
    row_id: Option<RowId>,
    raw_override: &dyn arrow::array::Array,
) -> egui::Response {
    let blueprint_path_clone = blueprint_path.clone();
    ui.list_item_flat_noninteractive(
        list_item::PropertyContent::new(name)
            .value_fn(|ui, _style| {
                let allow_multiline = false;
                query_ctx.viewer_ctx().component_ui_registry().edit_ui_raw(
                    query_ctx,
                    ui,
                    query_ctx.viewer_ctx().blueprint_db(),
                    blueprint_path_clone,
                    component_descr,
                    row_id,
                    raw_override,
                    allow_multiline,
                );
            })
            .with_action_button(&re_ui::icons::CLOSE, "Clear blueprint component", || {
                query_ctx
                    .viewer_ctx()
                    .clear_blueprint_component(blueprint_path, component_descr.clone());
            }),
    )
}

/// "More" menu for a component line in the visualizer ui.
#[expect(clippy::too_many_arguments)]
fn menu_more(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_descr: ComponentDescriptor,
    override_path: &EntityPath,
    raw_override: &Option<ArrayRef>,
    raw_default: Option<ArrayRef>,
    raw_fallback: ArrayRef,
    raw_current_value: ArrayRef,
) {
    remove_and_reset_override_buttons(
        ctx,
        ui,
        component_descr.clone(),
        override_path,
        raw_override,
    );

    if ui
        .add_enabled(
            raw_default.is_some(),
            egui::Button::new("Set to view default value"),
        )
        .on_disabled_hover_text("There's no default component active")
        .clicked()
    {
        if let Some(raw_default) = raw_default {
            ctx.save_blueprint_array(override_path.clone(), component_descr, raw_default);
        }
        ui.close();
        return;
    }

    if ui.button("Set to fallback value").clicked() {
        ctx.save_blueprint_array(override_path.clone(), component_descr, raw_fallback);
        ui.close();
        return;
    }

    if ui.button("Make default for current view").clicked() {
        ctx.save_blueprint_array(
            ViewBlueprint::defaults_path(ctx.view_id),
            component_descr,
            raw_current_value,
        );
        ui.close();
    }
}

pub fn remove_and_reset_override_buttons(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_descr: ComponentDescriptor,
    override_path: &EntityPath,
    raw_override: &Option<ArrayRef>,
) {
    if ui
        .add_enabled(raw_override.is_some(), egui::Button::new("Remove override"))
        .on_disabled_hover_text("There's no override active")
        .clicked()
    {
        ctx.clear_blueprint_component(override_path.clone(), component_descr);
        ui.close();
        return;
    }

    let override_differs_from_default = raw_override
        != &ctx
            .viewer_ctx
            .raw_latest_at_in_default_blueprint(override_path, component_descr.component);
    if ui
        .add_enabled(
            override_differs_from_default,
            egui::Button::new("Reset override to default blueprint"),
        )
        .on_hover_text("Resets the override to what is specified in the default blueprint")
        .on_disabled_hover_text("Current override is the same as the override specified in the default blueprint (if any)")
        .clicked()
    {
        ctx.reset_blueprint_component(override_path.clone(), component_descr.clone());
        ui.close();
    }
}

fn menu_add_new_visualizer(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[VisualizerInstruction],
    available_visualizers: &[ViewSystemIdentifier],
) {
    let override_base_path = data_result.override_base_path();

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    for visualizer_type in available_visualizers {
        if ui.button(visualizer_type.as_str()).clicked() {
            // To add a visualizer we have to do two things:
            // * add a visualizer type information for that new visualizer instruction
            // * add an element to the list of active visualizer ids
            let new_instruction = VisualizerInstruction::new(
                VisualizerInstructionId::new_random(),
                *visualizer_type,
                override_base_path,
                re_viewer_context::VisualizerComponentMappings::default(),
            );
            let active_visualizer_archetype = ActiveVisualizers::new(
                active_visualizers
                    .iter()
                    .map(|v| &v.id)
                    .chain(std::iter::once(&new_instruction.id))
                    .map(|v| v.0),
            );

            // If this is the first time we log `ActiveVisualizers`, we have to write out the instructions for all
            // visualizers which would be entirely heuristically generated at this point!
            let did_not_yet_persist_active_visualizers = ctx
                .blueprint_db()
                .latest_at(
                    ctx.blueprint_query(),
                    override_base_path,
                    ActiveVisualizers::all_components()
                        .iter()
                        .map(|c| c.component),
                )
                .components
                .is_empty();
            if did_not_yet_persist_active_visualizers {
                for instruction in active_visualizers {
                    instruction.write_instruction_to_blueprint(ctx.viewer_ctx);
                }
            }

            ctx.save_blueprint_archetype(override_base_path.clone(), &active_visualizer_archetype);
            new_instruction.write_instruction_to_blueprint(ctx.viewer_ctx);

            ui.close();
        }
    }
}

/// Lists all visualizers that are _not_ active for the given entity but could be.
fn available_inactive_visualizers(
    ctx: &ViewContext<'_>,
    data_result: &DataResult,
) -> Vec<ViewSystemIdentifier> {
    let view_class = ctx.view_class_entry();

    ctx.viewer_ctx
        .iter_visualizable_entities_for_view_class(view_class.identifier)
        .filter(|(_, visualizable_ents)| visualizable_ents.contains_key(&data_result.entity_path))
        .map(|(vis, _)| vis)
        .sorted()
        .collect::<Vec<_>>()
}
