use std::sync::Arc;

use itertools::{Either, Itertools as _};
use re_chunk::ComponentIdentifier;
use re_data_ui::{DataUi as _, sorted_component_list_by_archetype_for_ui};
use re_log_types::EntityPath;
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes::ActiveVisualizers;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_types_core::ComponentDescriptor;
use re_types_core::external::arrow::array::ArrayRef;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{ComboItem, OnResponseExt as _, UiExt as _, design_tokens_of_visuals, list_item};
use re_view::{
    AnnotationMapCache, SourceMappingContext, SourceSelectorContext, raw_default_or_fallback,
    raw_default_without_fallback, save_component_mapping, source_selector_ui,
    visualizers_for_entity,
};
use re_viewer_context::{
    DataResult, DatatypeMatch, RecommendedMappings, TryShowEditUiResult, UiLayout, ViewContext,
    ViewSystemIdentifier, VisualizableReason, VisualizerCollection, VisualizerComponentMappings,
    VisualizerComponentSource, VisualizerInstruction, VisualizerSystem, VisualizerViewReport,
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
a variety of sources. Use the source selector to choose where a component's value comes from.

A component can use one of the following sources:
- **Recording component**: A component logged on this entity. The source may be the component the \
visualizer normally uses or another compatible component selected in the UI.
- **Annotation context**: A color or label resolved from class and keypoint IDs when the visualizer \
supports annotation context.
- **Custom**: A value set in the UI and stored in the blueprint for this visualizer.
- **View default**: A value set for the current view. If none was set, the visualizer provides a \
context-sensitive default.

When no source has been selected explicitly, the Viewer automatically chooses an available source, \
preferring recording data and annotation context before the view default.";

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
        let response = ui.small_icon_button(&re_ui::icons::CLOSE, "Remove visualizer");
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

                    let has_custom_ui_for_components = visualizer.selection_ui(
                        ctx,
                        ui,
                        data_result,
                        visualizer_instruction,
                        per_type_visualizer_reports.get(&visualizer_type),
                    );
                    if !has_custom_ui_for_components {
                        visualizer_components(
                            ctx,
                            ui,
                            data_result,
                            visualizer,
                            visualizer_instruction,
                            per_type_visualizer_reports.get(&visualizer_type),
                        );
                    }
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

fn warn_for_missing_mapping(target_component: ComponentIdentifier) {
    re_log::debug_warn!(
        "No component source for component {}. Query results should **always** determine a source even if it is not reachable.",
        target_component
    );
}

fn visualizer_components(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    visualizer: &dyn VisualizerSystem,
    instruction: &VisualizerInstruction,
    type_report: Option<&re_viewer_context::VisualizerTypeReport>,
) {
    let selector_ctx =
        SourceSelectorContext::new(ctx, data_result, instruction, visualizer, type_report);
    let viewer_ctx = ctx.viewer_ctx;
    let query_result = &selector_ctx.query_result;
    let query_info = &selector_ctx.query_info;
    let query_ctx = query_result.query_context();

    // TODO(andreas): Should we show required components in a special way?
    for target_component_descr in sorted_component_list_by_archetype_for_ui(
        viewer_ctx.reflection(),
        query_info.queried.iter().cloned(),
    )
    .values()
    .flatten()
    {
        let target_component = target_component_descr.component;

        // Query override & default since we need them later on.
        let is_ui_editable = viewer_ctx
            .reflection()
            .field_reflection(target_component_descr)
            .is_some_and(|field| field.is_ui_editable());

        // Whether the component is required according to the query constraints to run the visualizer.
        let is_required = query_info
            .constraints
            .is_required_component(target_component);

        let raw_default = || -> ArrayRef {
            if is_ui_editable {
                raw_default_or_fallback(query_ctx, query_result, target_component_descr)
            } else {
                // In this context, we're only concerned with displaying an empty array, so it can be _any_ empty array.
                // This would have to change if we add data type information in this place to the UI as well.
                // Since our unified blueprint resolved query will still check the view defaults, we do so here too.
                raw_default_without_fallback(query_result, target_component_descr)
                    .unwrap_or_else(|| Arc::new(arrow::array::NullArray::new(0)))
            }
        };

        let (source, maybe_unit_chunk) = query_result
            .get_unit_chunk_with_source(target_component, true)
            .unwrap_or_else(|| {
                warn_for_missing_mapping(target_component);
                (&VisualizerComponentSource::Default, Ok(None))
            });

        let (current_value_row_id, raw_current_value_array) =
            if let Ok(Some(unit_chunk)) = &maybe_unit_chunk {
                if let Some((row_id, array)) =
                    unit_chunk.non_empty_component_batch_raw(target_component)
                {
                    (row_id, Some(array))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        // TODO(RR-3840): Today individual visualizers almost always fall back automatically to default values if the data is missing.
        // This should be handled automatically by the blueprint resolved query instead.
        // Since this is done by convention right now we have to emulate this convention here as well.
        let raw_current_value_array = if raw_current_value_array.is_some() || is_required {
            raw_current_value_array
        } else {
            Some(raw_default())
        };

        let component_reports: Vec<_> = type_report
            .into_iter()
            .flat_map(|r| r.reports_for_component(&instruction.id, target_component))
            .collect();
        let value_fn = |ui: &mut egui::Ui, _style| {
            let Some(raw_current_value_array) = &raw_current_value_array else {
                // There's no data, don't pretend otherwise by fetching a default (we've already handled all those cases earlier).
                if maybe_unit_chunk.is_ok()
                    || (matches!(&maybe_unit_chunk, Err(err) if err.is_data_temporarily_unavailable()))
                {
                    ui.weak("-");
                } else {
                    ui.label(egui::RichText::new("Missing").color(ui.tokens().error_fg_color));
                }
                return;
            };

            let multiline = false;
            if let TryShowEditUiResult::Shown { edited_value } =
                ctx.viewer_ctx.component_ui_registry().try_show_edit_ui(
                    &ctx.viewer_ctx.blueprint_store_view_ctx(),
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
                let store_view_ctx = ctx.viewer_ctx.active_recording_store_view_context();
                ctx.viewer_ctx.component_ui_registry().component_ui_raw(
                    &store_view_ctx,
                    ui,
                    UiLayout::List,
                    &data_result.entity_path,
                    target_component_descr,
                    current_value_row_id,
                    raw_current_value_array,
                );
            }
        };

        let annotation_map = AnnotationMapCache::for_query(ctx.viewer_ctx, &query_ctx.query);
        let annotations = annotation_map.find(&data_result.entity_path);
        let add_children = |ui: &mut egui::Ui| {
            let raw_default = raw_default();
            let mapping_ctx = SourceMappingContext {
                data_result,
                query_ctx,
                target_component_descr,
                is_ui_editable,
                instruction,
                source,
                raw_default: &raw_default,
                annotations,
            };
            // Source component (if available).
            source_selector_ui(
                ui,
                "Source",
                &mapping_ctx,
                &selector_ctx.entity_components_with_datatype,
                query_info,
                true,
                maybe_unit_chunk.as_ref().err().copied(),
                &component_reports,
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
        if is_ui_editable && let Some(raw_current_value_array) = &raw_current_value_array {
            property_content = property_content.with_menu_button(
                &re_ui::icons::MORE,
                "More options",
                |ui: &mut egui::Ui| {
                    menu_more(
                        ctx,
                        ui,
                        target_component_descr.clone(),
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
                component_type.data_ui(
                    &ctx.viewer_ctx.active_recording_store_view_context(),
                    ui,
                    UiLayout::Tooltip,
                );
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
    match report.diagnostic.severity {
        re_viewer_context::ViewerReportSeverity::Error => {
            let label = ui.error_label(&report.diagnostic.summary);
            if let Some(details) = &report.diagnostic.details {
                label.on_hover_text(details);
            }
        }
        re_viewer_context::ViewerReportSeverity::Warning => {
            let label = ui.warning_label(&report.diagnostic.summary);
            if let Some(details) = &report.diagnostic.details {
                label.on_hover_text(details);
            }
        }
        re_viewer_context::ViewerReportSeverity::Info => {
            let label = ui.info_label(&report.diagnostic.summary);
            if let Some(details) = &report.diagnostic.details {
                label.on_hover_text(details);
            }
        }
    }
}

/// "More" menu for a component line in the visualizer ui.
fn menu_more(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    component_descr: ComponentDescriptor,
    raw_current_value: ArrayRef,
) {
    if ui.button("Make default for current view").clicked() {
        ctx.save_blueprint_array(
            ViewBlueprint::defaults_path(ctx.view_id),
            component_descr,
            raw_current_value,
        );
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
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    // Determine which visualizers are recommended.
    let visualizers_with_reason = visualizers_for_entity(
        ctx.viewer_ctx,
        ctx.view_class_identifier,
        &data_result.entity_path,
    );
    let recommended_visualizers = ctx.view_class().recommended_visualizers_for_entity(
        &data_result.entity_path,
        &visualizers_with_reason,
        ctx.viewer_ctx.indicated_entities_per_visualizer,
    );

    let (recommended, other): (Vec<_>, Vec<_>) =
        available_visualizers.iter().copied().partition(|vis| {
            recommended_visualizers
                .all_recommendations()
                .contains_key(vis)
        });

    // Don't show categorization if either group is empty.
    let show_sections = !recommended.is_empty() && !other.is_empty();

    if show_sections {
        ui.add(re_ui::ComboItemHeader::new("Recommended:"));
    }
    for visualizer_type in &recommended {
        add_new_visualizer_button(ctx, ui, data_result, active_visualizers, *visualizer_type);
    }

    if show_sections {
        ui.add(re_ui::ComboItemHeader::new("Other:"));
    }
    for visualizer_type in &other {
        add_new_visualizer_button(ctx, ui, data_result, active_visualizers, *visualizer_type);
    }
}

fn add_new_visualizer_button(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    active_visualizers: &[VisualizerInstruction],
    visualizer_type: ViewSystemIdentifier,
) {
    let override_base_path = data_result.override_base_path();

    let already_active = active_visualizers
        .iter()
        .any(|v| v.visualizer_type == visualizer_type);

    if ui
        .add(ComboItem::new(visualizer_type.as_str()).selected(already_active))
        .clicked()
    {
        let component_mappings = component_mappings_for_new_visualizer(
            ctx,
            active_visualizers,
            &visualizer_type,
            &data_result.entity_path,
        );

        // To add a visualizer we have to do two things:
        // * add a visualizer type information for that new visualizer instruction
        // * add an element to the list of active visualizer ids
        let new_instruction = VisualizerInstruction::new(
            VisualizerInstructionId::new_random(),
            visualizer_type,
            override_base_path,
            component_mappings,
        );
        let active_visualizer_archetype = ActiveVisualizers::new(
            std::iter::chain(
                active_visualizers.iter().map(|v| &v.id),
                std::iter::once(&new_instruction.id),
            )
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

fn component_mappings_for_new_visualizer(
    ctx: &ViewContext<'_>,
    active_visualizers: &[VisualizerInstruction],
    visualizer_type: &ViewSystemIdentifier,
    entity_path: &EntityPath,
) -> VisualizerComponentMappings {
    // Get recommended visualizers with their component mappings so we can use them
    // when the user adds a new visualizer.
    let entity_visualizers =
        visualizers_for_entity(ctx.viewer_ctx, ctx.view_class_identifier, entity_path);
    let recommended_visualizers = ctx.view_class().recommended_visualizers_for_entity(
        entity_path,
        &entity_visualizers,
        ctx.viewer_ctx.indicated_entities_per_visualizer,
    );
    let component_mapping_recommendations = recommended_visualizers
        .all_recommendations()
        .get(visualizer_type)
        .cloned();

    // Chain in all possible mappings.
    let visualizable_reason = entity_visualizers
        .iter()
        .find(|(viz, _)| viz == visualizer_type)
        .map(|(_, reason)| *reason);
    let all_mapping_candidates = std::iter::chain(
        component_mapping_recommendations.into_iter().flatten(),
        component_mappings_for_required_components_from_visualizability(
            entity_path,
            visualizer_type,
            visualizable_reason,
        )
        .into_iter()
        .map(RecommendedMappings::from_mappings),
    );

    // Now out of this list of all mappings, pick the best one!
    //
    // Reminder: Complex prioritization is already done for recommended visualizers, so we only should do very loose prioritization beyond that!
    pick_best_mappings(
        all_mapping_candidates,
        active_visualizers,
        visualizable_reason,
    )
}

/// Picks the most suitable mapping out of all candidate mappings for a newly added visualizer.
fn pick_best_mappings(
    candidates: impl Iterator<Item = RecommendedMappings>,
    active_visualizers: &[VisualizerInstruction],
    visualizable_reason: Option<&VisualizableReason>,
) -> VisualizerComponentMappings {
    candidates
        .min_by_key(|recommended_mappings| {
            let is_trivial_mapping = recommended_mappings.mappings().is_empty()
                || recommended_mappings
                    .mappings()
                    .iter()
                    .all(|(target, source)| source.is_identity_mapping(*target));
            let is_already_in_use = active_visualizers.iter().any(|active_visualizer| {
                recommended_mappings.is_covered_by(&active_visualizer.component_mappings)
            });

            // A component that merely has the right arrow datatype is a much worse source than one
            // with the semantics the visualizer natively works with:
            // e.g. `Points3D:positions` should map from `GaussianSplats3D:centers` (`Position3D`),
            // not from `GaussianSplats3D:scales` (`Scale3D`), despite both sharing the
            // `rerun.encodings.Vec3D` encoding.
            let has_physical_only_source = visualizable_reason.is_some_and(|reason| {
                recommended_mappings.mappings().values().any(|source| {
                    matches!(
                        source,
                        VisualizerComponentSource::SourceComponent {
                            source_component, ..
                        } if reason.physical_datatype_only_match(*source_component)
                    )
                })
            });

            (
                is_already_in_use,                   // prefer mappings that haven't shown up yet
                has_physical_only_source, // prefer native semantics over plain datatype matches
                !is_trivial_mapping, // prefer mappings that are completely trivial (false sorts earlier)
                recommended_mappings.display_name(), // tie breaker, so that the pick is deterministic
            )
        })
        .map(RecommendedMappings::into_mappings)
        .unwrap_or_default()
}

/// Derives component mappings from the visualizability reason when no explicit recommendation exists.
fn component_mappings_for_required_components_from_visualizability(
    entity_path: &EntityPath,
    visualizer_type: &ViewSystemIdentifier,
    reason: Option<&VisualizableReason>,
) -> Vec<VisualizerComponentMappings> {
    match reason {
        Some(VisualizableReason::SingleRequiredComponentMatch(matches)) => matches
            .matches
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

                _ => Either::Right(std::iter::once(VisualizerComponentSource::simple_map(
                    *source_component,
                ))),
            })
            .map(|mapping| std::iter::once((matches.target_component, mapping)).collect())
            .collect(),

        Some(VisualizableReason::BufferAndFormatMatch(matches)) => {
            // Each (buffer_source, format_component) pair produces a candidate mapping set.
            // Buffer matches with nested field selectors expand into multiple candidates.
            let format_mappings: Vec<_> = matches
                .format_matches
                .iter()
                .map(|format_component| VisualizerComponentSource::simple_map(*format_component))
                .collect();

            matches
                .buffer_matches
                .iter()
                .flat_map(|(source_component, match_info)| {
                    let buffer_sources: Vec<_> = match match_info {
                        DatatypeMatch::PhysicalDatatypeOnly { selectors, .. }
                            if !selectors.is_empty() =>
                        {
                            selectors
                                .iter()
                                .map(|(selector, _)| VisualizerComponentSource::SourceComponent {
                                    source_component: *source_component,
                                    selector: selector.to_string(),
                                })
                                .collect()
                        }
                        _ => vec![VisualizerComponentSource::simple_map(*source_component)],
                    };
                    buffer_sources.into_iter().flat_map(|buffer_mapping| {
                        format_mappings.iter().map(move |format_mapping| {
                            [
                                (matches.buffer_target, buffer_mapping.clone()),
                                (matches.format_target, format_mapping.clone()),
                            ]
                            .into_iter()
                            .collect()
                        })
                    })
                })
                .collect()
        }

        // For non-datatype-match reasons (ExactMatchAny, Always),
        // the default identity mapping is correct as it will pick in builtin components.
        Some(VisualizableReason::ExactMatchAny | VisualizableReason::Always) => {
            vec![VisualizerComponentMappings::default()]
        }

        None => {
            re_log::debug_panic!(
                "Entity {entity_path:?} is not visualizable for {visualizer_type:?}, but was offered as an available visualizer"
            );
            re_log::warn_once!(
                "Entity {entity_path:?} is not visualizable for {visualizer_type:?}, but was offered as an available visualizer"
            );
            vec![VisualizerComponentMappings::default()]
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

#[cfg(test)]
mod tests {
    use super::*;

    use arrow::datatypes::DataType;
    use re_sdk_types::archetypes::{GaussianSplats3D, Points3D};
    use re_viewer_context::{DatatypeMatch, SingleRequiredComponentMatch};

    fn positions() -> ComponentIdentifier {
        Points3D::descriptor_positions().component
    }

    fn centers() -> ComponentIdentifier {
        GaussianSplats3D::descriptor_centers().component
    }

    fn scales() -> ComponentIdentifier {
        GaussianSplats3D::descriptor_scales().component
    }

    fn vec3d() -> DataType {
        <re_sdk_types::encodings::Vec3D as re_types_core::ArrowDataType>::arrow_data_type()
    }

    fn native(component: ComponentIdentifier) -> (ComponentIdentifier, DatatypeMatch) {
        (
            component,
            DatatypeMatch::NativeSemantics {
                arrow_data_type: vec3d(),
                component_type: Points3D::descriptor_positions().component_type,
            },
        )
    }

    fn physical_only(component: ComponentIdentifier) -> (ComponentIdentifier, DatatypeMatch) {
        (
            component,
            DatatypeMatch::PhysicalDatatypeOnly {
                arrow_data_type: vec3d(),
                component_type: GaussianSplats3D::descriptor_scales().component_type,
                selectors: Vec::new(),
            },
        )
    }

    /// A `Vec3D` column without any semantic type, as logged via `AnyValues`.
    fn untyped_physical_only(
        component: ComponentIdentifier,
    ) -> (ComponentIdentifier, DatatypeMatch) {
        (
            component,
            DatatypeMatch::PhysicalDatatypeOnly {
                arrow_data_type: vec3d(),
                component_type: None,
                selectors: Vec::new(),
            },
        )
    }

    fn positions_match(
        matches: impl IntoIterator<Item = (ComponentIdentifier, DatatypeMatch)>,
    ) -> VisualizableReason {
        VisualizableReason::SingleRequiredComponentMatch(SingleRequiredComponentMatch {
            target_component: positions(),
            matches: matches.into_iter().collect(),
        })
    }

    fn source(component: ComponentIdentifier) -> VisualizerComponentSource {
        VisualizerComponentSource::simple_map(component)
    }

    /// A single `positions` mapping.
    fn positions_from(component: ComponentIdentifier) -> VisualizerComponentMappings {
        std::iter::once((positions(), source(component))).collect()
    }

    /// Picks the best mapping out of all candidates derived from the visualizability reason.
    fn pick(
        reason: &VisualizableReason,
        active_visualizers: &[VisualizerInstruction],
    ) -> VisualizerComponentMappings {
        let candidates = component_mappings_for_required_components_from_visualizability(
            &EntityPath::from("splats"),
            &ViewSystemIdentifier::from_static_str("Points3D"),
            Some(reason),
        )
        .into_iter()
        .map(RecommendedMappings::from_mappings);

        pick_best_mappings(candidates, active_visualizers, Some(reason))
    }

    fn active_visualizer(mappings: VisualizerComponentMappings) -> VisualizerInstruction {
        VisualizerInstruction::new(
            VisualizerInstructionId::new_random(),
            ViewSystemIdentifier::from_static_str("Points3D"),
            &EntityPath::from("blueprint/visualizers"),
            mappings,
        )
    }

    /// Regression test for RR-5303: `Points3D:positions` must map from the `Position3D`-typed
    /// `centers`, not from the `Scale3D`-typed `scales` which merely has the same `Vec3D` datatype.
    #[test]
    fn native_semantics_wins_over_physical_datatype_match() {
        // Note that `CustomSplats:acceleration` would win any name-based tie breaking.
        let reason = positions_match([
            native(centers()),
            physical_only(scales()),
            untyped_physical_only(ComponentIdentifier::from_static_str(
                "CustomSplats:acceleration",
            )),
        ]);
        assert_eq!(pick(&reason, &[]), positions_from(centers()));
    }

    /// Without any native match we still fall back to a plain physical datatype match.
    #[test]
    fn physical_datatype_match_is_used_if_there_is_no_native_one() {
        let reason = positions_match([physical_only(scales())]);
        assert_eq!(pick(&reason, &[]), positions_from(scales()));
    }

    /// An identity mapping is native by construction and should be preferred.
    #[test]
    fn identity_mapping_is_preferred() {
        let reason = positions_match([
            native(positions()),
            native(centers()),
            physical_only(scales()),
        ]);
        assert_eq!(pick(&reason, &[]), positions_from(positions()));
    }

    /// Ties between equally good candidates are broken deterministically.
    #[test]
    fn pick_is_deterministic_among_equally_good_candidates() {
        // Both are native matches, so the tie is broken by name.
        let other_centers = ComponentIdentifier::from_static_str("AaaPoints:centers");
        let reason = positions_match([native(centers()), native(other_centers)]);

        // Same result no matter how often we ask (`matches` is a hash map, so iteration order varies).
        for _ in 0..10 {
            assert_eq!(pick(&reason, &[]), positions_from(other_centers));
        }
    }

    /// Mappings that are already in use are skipped, even if they'd be the better match.
    #[test]
    fn already_used_mapping_is_skipped() {
        let reason = positions_match([native(centers()), physical_only(scales())]);
        let active = [active_visualizer(positions_from(centers()))];
        assert_eq!(pick(&reason, &active), positions_from(scales()));
    }

    /// Sources that aren't part of the match at all (e.g. mappings for other slots coming from a
    /// view class recommendation) must not count as physical-only matches.
    #[test]
    fn mappings_for_unrelated_slots_are_not_penalized() {
        let colors = Points3D::descriptor_colors().component;
        let reason = positions_match([native(centers()), physical_only(scales())]);

        let native_plus_unrelated: VisualizerComponentMappings = [
            (positions(), source(centers())),
            (
                colors,
                source(GaussianSplats3D::descriptor_colors().component),
            ),
        ]
        .into_iter()
        .collect();
        let physical_only_mapping = positions_from(scales());

        let picked = pick_best_mappings(
            [
                RecommendedMappings::from_mappings(physical_only_mapping),
                RecommendedMappings::from_mappings(native_plus_unrelated.clone()),
            ]
            .into_iter(),
            &[],
            Some(&reason),
        );
        assert_eq!(picked, native_plus_unrelated);
    }

    #[test]
    fn no_candidates_yields_no_mappings() {
        assert_eq!(
            pick_best_mappings(std::iter::empty(), &[], None),
            VisualizerComponentMappings::default()
        );
    }
}
