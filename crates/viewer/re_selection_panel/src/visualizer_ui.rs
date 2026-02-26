use std::sync::Arc;

use arrow::datatypes::DataType;
use egui::Ui;
use itertools::{Either, Itertools as _};
use re_chunk::ComponentIdentifier;
use re_data_ui::{DataUi as _, sorted_component_list_by_archetype_for_ui};
use re_log_types::EntityPath;
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes::ActiveVisualizers;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::blueprint::datatypes::ComponentSourceKind;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_types_core::ComponentDescriptor;
use re_types_core::external::arrow::array::ArrayRef;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::menu::menu_style;
use re_ui::{ComboItem, OnResponseExt as _, UiExt as _, design_tokens_of_visuals, list_item};
use re_view::{
    BlueprintResolvedResultsExt as _, ChunksWithComponent, latest_at_with_blueprint_resolved_data,
};
use re_viewer_context::{
    AnyPhysicalDatatypeRequirement, BlueprintContext as _, DataResult, DatatypeMatch,
    PerVisualizerTypeInViewClass, TryShowEditUiResult, UiLayout, ViewContext, ViewSystemIdentifier,
    VisualizableEntities, VisualizableReason, VisualizerCollection, VisualizerComponentMappings,
    VisualizerComponentSource, VisualizerInstruction, VisualizerQueryInfo,
    VisualizerReportSeverity, VisualizerSystem, VisualizerViewReport,
};
use re_viewport_blueprint::ViewBlueprint;

pub fn visualizer_ui(
    ctx: &ViewContext<'_>,
    view: &ViewBlueprint,
    visualizer_errors: &VisualizerViewReport,
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
    let view_visualizers = ctx.new_visualizer_collection();
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
                &view_visualizers,
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
    per_type_visualizer_reports: &VisualizerViewReport,
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
                // List all components that the visualizer consumes.
                if let Ok(visualizer) = all_visualizers.get_by_type_identifier(visualizer_type) {
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

                    // Show reports that are not associated with a specific component at the top.
                    if let Some(reports) = per_type_visualizer_reports.get(&visualizer_type) {
                        for report in reports.reports_without_component(&visualizer_instruction.id)
                        {
                            show_visualizer_report(ui, report);
                        }
                    }

                    visualizer_components(
                        ctx,
                        ui,
                        data_result,
                        visualizer,
                        visualizer_instruction,
                        per_type_visualizer_reports.get(&visualizer_type),
                    );
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

fn visualizer_components(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    visualizer: &dyn VisualizerSystem,
    instruction: &VisualizerInstruction,
    type_report: Option<&re_viewer_context::VisualizerTypeReport>,
) {
    let query_info = visualizer.visualizer_query_info(ctx.viewer_ctx.app_options());

    let store_query = ctx.current_query();
    let viewer_ctx = ctx.viewer_ctx;
    let query_ctx = ctx.query_context(data_result, store_query.clone(), instruction.id);

    // Query fully resolved data.
    let query_result = latest_at_with_blueprint_resolved_data(
        ctx,
        None, // TODO(andreas): Figure out how to deal with annotation context here.
        &store_query,
        data_result,
        query_info.queried_components(),
        Some(instruction),
    );

    // Query all components of the entity so we can show them in the source component mapping UI.
    let entity_components_with_datatype = {
        let components = viewer_ctx
            .recording_engine()
            .store()
            .all_components_for_entity_sorted(&data_result.entity_path)
            .unwrap_or_default();
        components
            .into_iter()
            .filter_map(|component_id| {
                let component_type = viewer_ctx
                    .recording_engine()
                    .store()
                    .lookup_component_type(&data_result.entity_path, component_id);
                component_type.map(|(_, arrow_datatype)| (component_id, arrow_datatype))
            })
            .collect::<Vec<_>>()
    };

    // TODO(andreas): Should we show required components in a special way?
    for target_component_descr in sorted_component_list_by_archetype_for_ui(
        viewer_ctx.reflection(),
        query_info.queried.iter().cloned(),
    )
    .values()
    .flatten()
    {
        // TODO(andreas): What about annotation context?
        let target_component = target_component_descr.component;

        // Query override & default since we need them later on.
        let is_ui_editable = viewer_ctx
            .reflection()
            .field_reflection(target_component_descr)
            .is_some_and(|field| field.is_ui_editable());

        let raw_default = || -> ArrayRef {
            if is_ui_editable {
                raw_default_or_fallback(&query_ctx, &query_result, target_component_descr)
            } else {
                // In this context, we're only concerned with displaying an empty array, so it can be _any_ empty array.
                // This would have to change if we add data type information in this place to the UI as well.
                // Since our unified blueprint resolved query will still check the view defaults, we do so here too.
                raw_default_without_fallback(&query_result, target_component_descr)
                    .unwrap_or_else(|| Arc::new(arrow::array::NullArray::new(0)))
            }
        };

        // Current value as a raw arrow array + row id + error if any.
        // We're only interested in a single row, so first chunk is always enough.
        let force_preserve_row_ids = true;
        let chunks = query_result.get_chunks(target_component, force_preserve_row_ids);
        let (current_value_row_id, raw_current_value_array, mapping_error) =
            match ChunksWithComponent::try_from(chunks) {
                Ok(chunks) => {
                    let row_id_and_non_empty_raw_array = chunks.chunks.first().and_then(|chunk| {
                        let unit_chunk = chunk.clone().into_unit();
                        re_log::debug_assert!(
                            unit_chunk.is_some(),
                            "Expected unit chunk from latest-at query"
                        );
                        unit_chunk?.non_empty_component_batch_raw(target_component)
                    });

                    // If there's no value, or the array is empty, use the fallback for display since this is what the visualizer _should_ use.
                    if let Some((current_value_row_id, raw_current_value_array)) =
                        row_id_and_non_empty_raw_array
                    {
                        (current_value_row_id, raw_current_value_array, None)
                    } else {
                        (None, raw_default(), None)
                    }
                }

                Err(err) => (None, raw_default(), Some(err)),
            };

        // Any mapping errors should already be in the `component_reports` below, since the visualizers should
        // fail in the exact same way. So the mapping errors can be explicitly ignored:
        let _mapping_err = mapping_error;

        let component_reports: Vec<_> = type_report
            .into_iter()
            .flat_map(|r| r.reports_for_component(&instruction.id, target_component))
            .collect();

        let value_fn = |ui: &mut egui::Ui, _style| {
            let multiline = false;
            if let TryShowEditUiResult::Shown { edited_value } =
                ctx.viewer_ctx.component_ui_registry().try_show_edit_ui(
                    ctx.viewer_ctx,
                    ui,
                    re_viewer_context::EditTarget {
                        store_id: ctx.viewer_ctx.store_context.blueprint.store_id().clone(),
                        timepoint: ctx
                            .viewer_ctx
                            .store_context
                            .blueprint_timepoint_for_writes(),
                        entity_path: instruction.override_path.clone(),
                    },
                    raw_current_value_array.as_ref(),
                    target_component_descr.clone(),
                    multiline,
                )
            {
                if edited_value {
                    // Make sure we're in override mode.
                    save_component_mapping(
                        ctx,
                        instruction,
                        VisualizerComponentSource::Override,
                        target_component,
                    );
                }
            } else {
                // Display the value without edit ui.
                ctx.viewer_ctx.component_ui_registry().component_ui_raw(
                    ctx.viewer_ctx,
                    ui,
                    UiLayout::List,
                    &store_query,
                    ctx.recording(),
                    &data_result.entity_path,
                    target_component_descr,
                    current_value_row_id,
                    &raw_current_value_array,
                );
            }
        };

        let add_children = |ui: &mut egui::Ui| {
            let raw_default = raw_default();
            let mapping_ctx = SourceMappingContext {
                data_result,
                query_ctx: query_result.query_context(),
                target_component_descr,
                is_ui_editable,
                instruction,
                raw_default: &raw_default,
            };
            // Source component (if available).
            source_component_ui(
                ui,
                &mapping_ctx,
                &query_result,
                &entity_components_with_datatype,
                &query_info,
                component_reports
                    .iter()
                    .find(|report| report.severity == VisualizerReportSeverity::Error)
                    .map(|report| report.summary.clone()),
            );
        };

        let default_open = false;

        let mut property_content = list_item::PropertyContent::new(
            // We're in the context of a visualizer, so we don't have to print the archetype name
            // since usually archetypes match 1:1 with visualizers.
            target_component_descr.archetype_field_name(),
        )
        .value_fn(value_fn)
        .show_only_when_collapsed(false)
        // TODO(emilk/egui#7531): Ideally we would hide the button unless hovered, but this
        // currently breaks the menu.
        .with_always_show_buttons(true);

        // Show the more options button only if we're ui editable. None of these options make sense otherwise.
        if is_ui_editable {
            property_content = property_content.with_menu_button(
                &re_ui::icons::MORE,
                "More options",
                |ui: &mut egui::Ui| {
                    menu_more(
                        ctx,
                        ui,
                        target_component_descr.clone(),
                        &instruction.override_path,
                        raw_current_value_array.clone(),
                    );
                },
            );
        }

        let response = ui
            .list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(target_component),
                default_open,
                property_content,
                add_children,
            )
            .item_response;

        if let Some(component_type) = target_component_descr.component_type {
            response.on_hover_ui(|ui| {
                // TODO(andreas): Add data ui for component descr?
                component_type.data_ui_recording(ctx.viewer_ctx, ui, UiLayout::Tooltip);
            });
        }

        for report in &component_reports {
            show_visualizer_report(ui, report);
        }
    }
}

fn show_visualizer_report(
    ui: &mut egui::Ui,
    report: &re_viewer_context::VisualizerInstructionReport,
) {
    match report.severity {
        re_viewer_context::VisualizerReportSeverity::OverallVisualizerError
        | re_viewer_context::VisualizerReportSeverity::Error => {
            let label = ui.error_label(&report.summary);
            if let Some(details) = &report.details {
                label.on_hover_text(details);
            }
        }
        re_viewer_context::VisualizerReportSeverity::Warning => {
            let label = ui.warning_label(&report.summary);
            if let Some(details) = &report.details {
                label.on_hover_text(details);
            }
        }
    }
}

fn raw_default_without_fallback(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    target_component_descr: &ComponentDescriptor,
) -> Option<Arc<dyn re_chunk::ArrowArray>> {
    let target_component = target_component_descr.component;

    let result_default = query_result.view_defaults.get(target_component)?;
    result_default
        .non_empty_component_batch_raw(target_component)
        .map(|(_, arr)| arr)
}

fn raw_default_or_fallback(
    query_ctx: &re_viewer_context::QueryContext<'_>,
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    target_component_descr: &ComponentDescriptor,
) -> Arc<dyn re_chunk::ArrowArray> {
    raw_default_without_fallback(query_result, target_component_descr).unwrap_or_else(|| {
        query_ctx
            .viewer_ctx()
            .component_fallback_registry
            .fallback_for(target_component_descr, query_ctx)
    })
}

fn collect_source_component_options(
    mapping_ctx: &SourceMappingContext<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
) -> Vec<VisualizerComponentSource> {
    let component_descr = mapping_ctx.target_component_descr;

    let no_mapping_mapping = VisualizerComponentSource::SourceComponent {
        source_component: component_descr.component,
        selector: String::new(),
    };

    let Some(target_component_type) = &component_descr.component_type else {
        return vec![no_mapping_mapping];
    };

    // Collect suitable source components with the same datatype as the target component.

    // TODO(andreas): Right now we are _more_ flexible for required components, because there we also support
    // casting in some special cases. Eventually this should always be the case, leaving us always with a list of valid physical types that we filter on.
    let allowed_physical_types = if let re_viewer_context::RequiredComponents::AnyPhysicalDatatype(
        AnyPhysicalDatatypeRequirement {
            target_component,
            physical_types,
            ..
        },
    ) = &query_info.required
        && target_component == &mapping_ctx.target_component()
    {
        physical_types.clone()
    } else {
        // Get arrow datatype of the target component.
        let reflection = mapping_ctx.viewer_ctx().reflection();
        let Some(target_component_reflection) = reflection.components.get(target_component_type)
        else {
            // No reflection for target component type, that should never happen.
            re_log::warn_once!(
                "No reflection information for visualizer target component type {:?} found. Unable to determine valid component mappings.",
                target_component_type
            );
            return Vec::new();
        };
        std::iter::once(target_component_reflection.datatype.clone()).collect()
    };

    entity_components_with_datatype
        .iter()
        .flat_map(|(source_component, datatype)| {
            use itertools::Either;

            let source_component = *source_component;

            // Direct match?
            if allowed_physical_types.contains(datatype) {
                Either::Left(Either::Left(std::iter::once(
                    VisualizerComponentSource::SourceComponent {
                        source_component,
                        selector: String::new(),
                    },
                )))
            }
            // Match fields in the struct?
            else if let Some(selectors) =
                re_arrow_combinators::extract_nested_fields(datatype, |dt| {
                    allowed_physical_types.contains(dt)
                })
            {
                Either::Left(Either::Right(selectors.into_iter().map(move |(sel, _)| {
                    VisualizerComponentSource::SourceComponent {
                        source_component,
                        selector: sel.to_string(),
                    }
                })))
            } else {
                Either::Right(std::iter::empty())
            }
        })
        .collect()
}

/// Context for rendering the source component mapping combo box items.
struct SourceMappingContext<'a> {
    data_result: &'a DataResult,
    query_ctx: &'a re_viewer_context::QueryContext<'a>,
    target_component_descr: &'a ComponentDescriptor,
    is_ui_editable: bool,
    instruction: &'a VisualizerInstruction,
    raw_default: &'a ArrayRef,
}

impl<'a> SourceMappingContext<'a> {
    fn view_ctx(&self) -> &ViewContext<'a> {
        self.query_ctx.view_ctx
    }

    fn viewer_ctx(&self) -> &re_viewer_context::ViewerContext<'a> {
        self.query_ctx.view_ctx.viewer_ctx
    }

    fn target_component(&self) -> ComponentIdentifier {
        self.target_component_descr.component
    }
}

fn source_component_ui(
    ui: &mut egui::Ui,
    mapping_ctx: &SourceMappingContext<'_>,
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
    current_selection_error: Option<String>,
) {
    let current = current_component_source(
        query_result,
        mapping_ctx.instruction,
        mapping_ctx.target_component(),
    );

    ui.push_id("source_component", |ui| {
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new("Source").value_fn(
            |ui, _| {
                let response = egui::ComboBox::new("source_component_combo_box", "")
                    .selected_text(component_source_string(&current))
                    .popup_style(menu_style())
                    .show_ui(ui, |ui| {
                        source_component_items_ui(
                            ui,
                            mapping_ctx,
                            entity_components_with_datatype,
                            query_info,
                            &current,
                            current_selection_error,
                        );
                    });
                response.response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::ComboBox,
                        ui.is_enabled(),
                        // TODO(aedm): Weird label, but we need to find this item in the integration test somehow.
                        format!("{}_$source", mapping_ctx.target_component()),
                    )
                });
            },
        ));
    });
}

fn source_component_items_ui(
    ui: &mut egui::Ui,
    mapping_ctx: &SourceMappingContext<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
    current: &VisualizerComponentSource,
    mut current_selection_error: Option<String>,
) {
    let mut options =
        collect_source_component_options(mapping_ctx, entity_components_with_datatype, query_info);

    let raw_override = mapping_ctx.viewer_ctx().raw_latest_at_in_current_blueprint(
        &mapping_ctx.instruction.override_path,
        mapping_ctx.target_component(),
    );

    if mapping_ctx.is_ui_editable {
        options.push(VisualizerComponentSource::Default);

        // Show the override only if we have one already.
        // (Otherwise, we'll add a special "add custom" entry later on)
        if raw_override.is_some() {
            options.push(VisualizerComponentSource::Override);
        }
    }

    // If the current source is not in the options list (e.g. because the selector is invalid
    // or the source component doesn't exist), add it so it still shows up as selected.
    if !options.contains(current) {
        options.insert(0, current.clone());
    }

    // Split options into recommended and other.
    let recommended_options = extract_recommended_source_options(mapping_ctx, &options);
    let other_options = options
        .into_iter()
        .filter(|option| !recommended_options.contains(option))
        .collect::<Vec<_>>();

    // Don't show categorization if either group is empty (ignoring Custom-only in "Other").
    let other_has_non_custom = other_options
        .iter()
        .any(|s| *s != VisualizerComponentSource::Override);
    let show_sections = !recommended_options.is_empty() && other_has_non_custom;

    if show_sections {
        ui.add(re_ui::ComboItemHeader::new("Recommended:"));
    }
    for source in &recommended_options {
        source_component_item_ui(
            ui,
            mapping_ctx,
            current,
            &mut current_selection_error,
            source,
        );
    }

    if show_sections {
        ui.add(re_ui::ComboItemHeader::new("Other values:"));
    }
    for source in &other_options {
        source_component_item_ui(
            ui,
            mapping_ctx,
            current,
            &mut current_selection_error,
            source,
        );
    }

    // Last: "Add Custom" if we don't have an override already, we're allowed to edit it and there's an editor ui available.
    let has_editor = mapping_ctx
        .target_component_descr
        .component_type
        .is_some_and(|ct| {
            mapping_ctx
                .viewer_ctx()
                .component_ui_registry()
                .registered_ui_types(ct)
                .has_edit_ui(mapping_ctx.raw_default.len() > 1)
        });
    if raw_override.is_none()
        && mapping_ctx.is_ui_editable
        && has_editor
        && ui.add(ComboItem::new("Add custom")).clicked()
    {
        save_component_mapping(
            mapping_ctx.view_ctx(),
            mapping_ctx.instruction,
            VisualizerComponentSource::Override,
            mapping_ctx.target_component(),
        );

        // Persist the override value right away, so the `add_custom` check can rely on the override value being in the blueprint store.
        // This also makes behavior generally more consistent - imagine what if the default flickers for some reason:
        // this will make it so that override doesn't flicker until one edits the value.
        mapping_ctx.view_ctx().save_blueprint_array(
            mapping_ctx.instruction.override_path.clone(),
            mapping_ctx.target_component_descr.clone(),
            mapping_ctx.raw_default.clone(),
        );

        ui.close();
    }
}

/// Determines which source component options should be in the "Recommended" group.
fn extract_recommended_source_options(
    mapping_ctx: &SourceMappingContext<'_>,
    options: &[VisualizerComponentSource],
) -> Vec<VisualizerComponentSource> {
    // Folks with Rerun access check https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=791-7619&t=6SWixKV9yWMTFQba-0
    // for the original design & rationale.

    let target_component = mapping_ctx.target_component();

    // Rule 1: Identity mapping is recommended.
    if options
        .iter()
        .any(|source| source.is_identity_mapping(target_component))
    {
        return vec![VisualizerComponentSource::identity(target_component)];
    }

    // Rule 2: View-recommended mappings are recommended.
    let view_ctx = mapping_ctx.view_ctx();
    let viewer_ctx = mapping_ctx.viewer_ctx();
    let visualizable_entities_per_visualizer =
        viewer_ctx.collect_visualizable_entities_for_view_class(view_ctx.view_class_identifier);
    let recommended_visualizers = view_ctx.view_class().recommended_visualizers_for_entity(
        &mapping_ctx.data_result.entity_path,
        &visualizable_entities_per_visualizer,
        viewer_ctx.indicated_entities_per_visualizer,
    );
    if let Some(recommended_mappings) = recommended_visualizers
        .0
        .get(&mapping_ctx.instruction.visualizer_type)
    {
        let recommended: Vec<_> = recommended_mappings
            .iter()
            .filter_map(|mappings| mappings.get_source_for_component(&target_component))
            .filter(|source| options.contains(source))
            .cloned()
            .collect();

        if !recommended.is_empty() {
            return recommended;
        }
    }

    // Rule 3: Default is recommended if present in the option list & non-empty.
    if !mapping_ctx.raw_default.is_empty() && options.contains(&VisualizerComponentSource::Default)
    {
        return vec![VisualizerComponentSource::Default];
    }

    // Otherwise: nothing is recommended.
    Vec::new()
}

/// Renders a single source component item in the combo box.
fn source_component_item_ui(
    ui: &mut egui::Ui,
    mapping_ctx: &SourceMappingContext<'_>,
    current: &VisualizerComponentSource,
    current_selection_error: &mut Option<String>,
    source: &VisualizerComponentSource,
) {
    let selected = source == current;

    let raw_value = raw_value_for_mapping(mapping_ctx, source);

    let mut item = ComboItem::new(component_source_string(source)).selected(selected);
    if selected && let Some(error) = current_selection_error.take() {
        item = item.error(Some(error));
    }

    if let Some(raw_value) = raw_value {
        let num_values = raw_value.len();
        item = item.value_widget(move |ui: &mut Ui| {
            // We intentionally don't show the value if there are multiple values since it can get cluttery. We'll likely iterate on this in the future.
            if num_values > 1 {
                ui.label(format!("{} values", re_format::format_uint(num_values)));
            } else {
                let viewer_ctx = mapping_ctx.viewer_ctx();
                viewer_ctx.component_ui_registry().component_ui_raw(
                    viewer_ctx,
                    ui,
                    UiLayout::List,
                    &mapping_ctx.query_ctx.query,
                    mapping_ctx.view_ctx().recording(),
                    &mapping_ctx.data_result.entity_path,
                    mapping_ctx.target_component_descr,
                    None, // row id doesn't matter since we're only showing a single value here.
                    &raw_value,
                );
            }
            ui.response()
        });
    }

    if ui.add(item).clicked() {
        save_component_mapping(
            mapping_ctx.view_ctx(),
            mapping_ctx.instruction,
            source.clone(),
            mapping_ctx.target_component(),
        );
        ui.close();
    }
}

fn raw_value_for_mapping(
    mapping_ctx: &SourceMappingContext<'_>,
    new_source: &VisualizerComponentSource,
) -> Option<Arc<dyn re_chunk::ArrowArray>> {
    let target_component = mapping_ctx.target_component();

    if new_source == &VisualizerComponentSource::Default {
        // Special treat for default, since it may also go to the fallback and we've already done that work.
        Some(mapping_ctx.raw_default.clone())
    } else {
        // Instead of trying to do an isolated query on this hypothetical source,
        // let's just pretend that the visualizer already took over this source, and see what the result would be!
        let hypothetical_instruction = VisualizerInstruction {
            component_mappings: std::iter::once((target_component, new_source.clone())).collect(),
            ..mapping_ctx.instruction.clone()
        };
        let query_result = latest_at_with_blueprint_resolved_data(
            mapping_ctx.view_ctx(),
            None,
            &mapping_ctx.query_ctx.query,
            mapping_ctx.data_result,
            [target_component],
            Some(&hypothetical_instruction),
        );
        query_result.get_raw_cell(target_component)
    }
}

/// Determines which component source is currently active.
///
/// If none is encoded in the visualizer instruction, we apply the same logic as `re_view::query`.
fn current_component_source(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    instruction: &VisualizerInstruction,
    component: ComponentIdentifier,
) -> VisualizerComponentSource {
    // Use explicit mapping if available.
    if let Some(mapping) = instruction.component_mappings.get(&component) {
        return mapping.clone();
    }

    // Otherwise check what the query did resolve to.
    match query_result.component_source_kind_for(component) {
        Some(Ok(ComponentSourceKind::SourceComponent)) => {
            // The query resolved to a source component, but there is no explicit mapping, so it must be a builtin source.
            VisualizerComponentSource::SourceComponent {
                source_component: component,
                selector: String::new(),
            }
        }
        Some(Ok(ComponentSourceKind::Override)) => VisualizerComponentSource::Override,
        Some(Ok(ComponentSourceKind::Default)) => VisualizerComponentSource::Default,
        Some(Err(_)) => {
            // There's no explicit mapping and there was a component mapping error. Can only mean that this was the standard source component.
            // TODO(andreas): Shaky argumentation. Override and default could also fail? If not now, maybe in the future?
            VisualizerComponentSource::SourceComponent {
                source_component: component,
                selector: String::new(),
            }
        }
        None => {
            re_log::debug_panic!(
                "Expected component {component:?} to be resolved to a source kind in the query result",
            );
            VisualizerComponentSource::Default
        }
    }
}

fn component_source_string(source: &VisualizerComponentSource) -> String {
    match source {
        VisualizerComponentSource::SourceComponent {
            source_component,
            selector,
        } => {
            if selector.is_empty() {
                source_component.as_str().to_owned()
            } else {
                format!("{}{}", source_component.as_str(), selector)
            }
        }
        VisualizerComponentSource::Override => "Custom".to_owned(),
        VisualizerComponentSource::Default => "View default".to_owned(),
    }
}

fn save_component_mapping(
    ctx: &ViewContext<'_>,
    instruction: &VisualizerInstruction,
    source_component: VisualizerComponentSource,
    target: ComponentIdentifier,
) {
    let mut updated_instruction = instruction.clone();

    // Set or override the mapping
    updated_instruction
        .component_mappings
        .insert(target, source_component);

    // TODO(andreas): Don't write the type if it hasn't changed
    updated_instruction.write_instruction_to_blueprint(ctx.viewer_ctx);
}

/// "More" menu for a component line in the visualizer ui.
fn menu_more(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_descr: ComponentDescriptor,
    override_path: &EntityPath,
    raw_current_value: ArrayRef,
) {
    reset_override_button(ctx, ui, component_descr.clone(), override_path);

    if ui.button("Make default for current view").clicked() {
        ctx.save_blueprint_array(
            ViewBlueprint::defaults_path(ctx.view_id),
            component_descr,
            raw_current_value,
        );
        ui.close();
    }
}

pub fn reset_override_button(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_descr: ComponentDescriptor,
    override_path: &EntityPath,
) {
    let component = component_descr.component;
    let raw_override = ctx
        .viewer_ctx
        .raw_latest_at_in_current_blueprint(override_path, component);
    let raw_override_default_blueprint = ctx
        .viewer_ctx
        .raw_latest_at_in_default_blueprint(override_path, component);

    if ui
        .add_enabled(
            raw_override != raw_override_default_blueprint,
            egui::Button::new("Reset override to default blueprint"),
        )
        .on_hover_text("Resets the override to what is specified in the default blueprint")
        .on_disabled_hover_text("Current override is the same as the override specified in the default blueprint (if any)")
        .clicked()
    {
        ctx.reset_blueprint_component(override_path.clone(), component_descr);
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
            let component_mappings = component_mappings_for_new_visualizer(
                ctx,
                active_visualizers,
                visualizer_type,
                &data_result.entity_path,
            );

            // To add a visualizer we have to do two things:
            // * add a visualizer type information for that new visualizer instruction
            // * add an element to the list of active visualizer ids
            let new_instruction = VisualizerInstruction::new(
                VisualizerInstructionId::new_random(),
                *visualizer_type,
                override_base_path,
                component_mappings,
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

/// Returns true if the proposed mapping is fully covered by an existing visualizer.
fn is_mapping_already_in_use(
    active_visualizers: &[VisualizerInstruction],
    mapping: &VisualizerComponentMappings,
) -> bool {
    active_visualizers.iter().any(|active_visualizer| {
        mapping.iter().all(|(mapping_src, mapping_target)| {
            active_visualizer.component_mappings.get(mapping_src) == Some(mapping_target)
        })
    })
}

fn component_mappings_for_new_visualizer(
    ctx: &ViewContext<'_>,
    active_visualizers: &[VisualizerInstruction],
    visualizer_type: &ViewSystemIdentifier,
    entity_path: &EntityPath,
) -> VisualizerComponentMappings {
    // Get recommended visualizers with their component mappings so we can use them
    // when the user adds a new visualizer.
    let visualizable_entities_per_visualizer = ctx
        .viewer_ctx
        .collect_visualizable_entities_for_view_class(ctx.view_class_identifier);
    let recommended_visualizers = ctx.view_class().recommended_visualizers_for_entity(
        entity_path,
        &visualizable_entities_per_visualizer,
        ctx.viewer_ctx.indicated_entities_per_visualizer,
    );
    let component_mapping_recommendations = recommended_visualizers.0.get(visualizer_type).cloned();

    // Chain in all possible mappings.
    let all_mapping_candidates = component_mapping_recommendations
        .into_iter()
        .flatten()
        .map(re_viewer_context::RecommendedMappings::into_mappings)
        .chain(
            component_mappings_for_required_components_from_visualizability(
                *visualizer_type,
                entity_path,
                &visualizable_entities_per_visualizer,
            ),
        );

    // Now out of this list of all mappings, pick the best one!
    //
    // Reminder: Complex prioritization is already done for recommended visualizers, so we only should do very loose prioritization beyond that!
    all_mapping_candidates
        .min_by_key(|mappings| {
            let is_trivial_mapping = mappings.is_empty()
                || mappings
                    .iter()
                    .all(|(target, source)| source.is_identity_mapping(*target));

            (
                is_mapping_already_in_use(active_visualizers, mappings), // prefer mappings that haven't shown up yet
                !is_trivial_mapping, // prefer mappings that are completely trivial (false sorts earlier)
            )
        })
        .unwrap_or_default()
}

/// Derives component mappings from the visualizability reason when no explicit recommendation exists.
fn component_mappings_for_required_components_from_visualizability(
    visualizer_type: ViewSystemIdentifier,
    entity_path: &EntityPath,
    visualizable_entities_per_visualizer: &PerVisualizerTypeInViewClass<VisualizableEntities>,
) -> impl Iterator<Item = VisualizerComponentMappings> {
    // Look up why this entity is visualizable for this visualizer type.
    let reason = visualizable_entities_per_visualizer
        .get(&visualizer_type)
        .and_then(|entities| entities.get(entity_path));

    let Some(VisualizableReason::DatatypeMatchAny {
        matches,
        target_component,
    }) = reason
    else {
        if reason.is_none() {
            re_log::debug_panic!(
                "Entity {entity_path:?} is not visualizable for {visualizer_type:?}, but was offered as an available visualizer"
            );
            re_log::warn_once!(
                "Entity {entity_path:?} is not visualizable for {visualizer_type:?}"
            );
        }
        // For non-datatype-match reasons (ExactMatchAll, ExactMatchAny, Always),
        // the default identity mapping is correct as it will pick in builtin components.
        return Either::Left(std::iter::once(VisualizerComponentMappings::default()));
    };

    // Set up an expression for all possible mappings given this visualization reason.
    Either::Right(
        matches
            .iter()
            .flat_map(|(source_component, match_info)| match match_info {
                DatatypeMatch::PhysicalDatatypeOnly { selectors, .. } if !selectors.is_empty() => {
                    Either::Left(selectors.iter().map(|(selector, _)| {
                        VisualizerComponentSource::SourceComponent {
                            source_component: *source_component,
                            selector: selector.to_string(),
                        }
                    }))
                }

                _ => Either::Right(std::iter::once(
                    VisualizerComponentSource::SourceComponent {
                        source_component: *source_component,
                        selector: String::new(),
                    },
                )),
            })
            .map(|mapping| std::iter::once((*target_component, mapping)).collect()),
    )
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
