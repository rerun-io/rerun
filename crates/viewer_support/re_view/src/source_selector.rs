//! Rendering the *source* of a component: where a visualizer's input comes from.
//!
//! Lives here rather than in the selection panel so that a view can render the
//! same UI without depending on a panel.

use std::borrow::Cow;
use std::sync::Arc;

use egui::Ui;

use arrow::datatypes::DataType;
use re_chunk::ComponentIdentifier;
use re_log_types::EntityPath;
use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_types_core::ComponentDescriptor;
use re_types_core::external::arrow::array::ArrayRef;
use re_ui::menu::menu_style;
use re_ui::{ComboItem, UiExt as _, UiLayout, list_item};
use re_viewer_context::BlueprintContext as _;
use re_viewer_context::{
    DataResult, ViewContext, ViewSystemIdentifier, ViewerReportSeverity, VisualizableReason,
    VisualizerComponentSource, VisualizerInstruction, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{AnnotationMapCache, ComponentMappingError, latest_at_with_blueprint_resolved_data};

/// Helper struct to render component source selector UI from `VisualizerSystem::selection_ui`.
///
/// Created once per `selection_ui` call; the precomputed query result and entity component
/// list are reused across each [`Self::source_selector_ui`] call.
pub struct SourceSelectorContext<'a> {
    ctx: &'a ViewContext<'a>,
    data_result: &'a DataResult,
    instruction: &'a VisualizerInstruction,
    type_report: Option<&'a re_viewer_context::VisualizerTypeReport>,
    pub query_info: VisualizerQueryInfo,
    pub query_result: crate::BlueprintResolvedLatestAtResults<'a>,
    pub entity_components_with_datatype: Vec<(ComponentIdentifier, DataType)>,
}

impl<'a> SourceSelectorContext<'a> {
    pub fn new(
        ctx: &'a ViewContext<'a>,
        data_result: &'a DataResult,
        instruction: &'a VisualizerInstruction,
        visualizer: &dyn VisualizerSystem,
        type_report: Option<&'a re_viewer_context::VisualizerTypeReport>,
    ) -> Self {
        let query_info = visualizer.visualizer_query_info(ctx.viewer_ctx.app_options());
        let store_query = ctx.current_query();
        let annotation_map = AnnotationMapCache::for_query(ctx.viewer_ctx, &store_query);
        let annotations = annotation_map.find(&data_result.entity_path);

        // Query fully resolved data.
        let query_result = latest_at_with_blueprint_resolved_data(
            ctx,
            annotations,
            &store_query,
            data_result,
            query_info.queried_components(),
            Some(instruction),
        );

        // Query all components of the entity so we can show them in the source component mapping UI.
        let entity_components_with_datatype = {
            let engine = ctx.viewer_ctx.recording_engine();
            let store = engine.store();
            let components = store
                .schema()
                .all_components_for_entity(&data_result.entity_path);
            components
                .into_iter()
                .flatten()
                .filter_map(|&component_id| {
                    let component_type = store
                        .schema()
                        .lookup_component_type(&data_result.entity_path, component_id);
                    component_type.map(|(_, arrow_data_type)| (component_id, arrow_data_type))
                })
                .collect::<Vec<_>>()
        };

        Self {
            ctx,
            data_result,
            instruction,
            type_report,
            query_info,
            query_result,
            entity_components_with_datatype,
        }
    }

    /// Shows a source-selector combo box for a single component.
    ///
    /// Set `show_default_and_override` to `false` for components whose value comes
    /// from a time-ranged query rather than a single latest-at value — "View default"
    /// and "Add custom" are then hidden because they wouldn't correspond to anything
    /// meaningful.
    pub fn source_selector_ui(
        &self,
        ui: &mut egui::Ui,
        target_component_descr: &ComponentDescriptor,
        show_default_and_override: bool,
    ) {
        let target_component = target_component_descr.component;
        let viewer_ctx = self.ctx.viewer_ctx;

        let Some((source, maybe_unit_chunk)) = self
            .query_result
            .get_unit_chunk_with_source(target_component, true)
        else {
            warn_for_missing_mapping(target_component);
            return;
        };

        let is_ui_editable = viewer_ctx
            .reflection()
            .field_reflection(target_component_descr)
            .is_some_and(|field| field.is_ui_editable());

        let raw_default = {
            if is_ui_editable {
                raw_default_or_fallback(
                    self.query_result.query_context(),
                    &self.query_result,
                    target_component_descr,
                )
            } else {
                raw_default_without_fallback(&self.query_result, target_component_descr)
                    .unwrap_or_else(|| Arc::new(arrow::array::NullArray::new(0)))
            }
        };

        let component_reports: Vec<_> = self
            .type_report
            .into_iter()
            .flat_map(|r| r.reports_for_component(&self.instruction.id, target_component))
            .collect();

        let annotation_map = AnnotationMapCache::for_query(
            self.ctx.viewer_ctx,
            &self.query_result.query_context().query,
        );
        let annotations = annotation_map.find(&self.data_result.entity_path);
        let mapping_ctx = SourceMappingContext {
            data_result: self.data_result,
            query_ctx: self.query_result.query_context(),
            target_component_descr,
            is_ui_editable,
            instruction: self.instruction,
            source,
            raw_default: &raw_default,
            annotations,
        };

        ui.push_id(target_component, |ui| {
            source_selector_ui(
                ui,
                target_component_descr.archetype_field_name(),
                &mapping_ctx,
                &self.entity_components_with_datatype,
                &self.query_info,
                show_default_and_override,
                maybe_unit_chunk.as_ref().err().copied(),
                &component_reports,
            );
        });
    }
}

fn warn_for_missing_mapping(target_component: ComponentIdentifier) {
    re_log::debug_warn!(
        "No component source for component {}. Query results should **always** determine a source even if it is not reachable.",
        target_component
    );
}

fn resolve_current_selection_error<'a>(
    mapping_error: Option<&ComponentMappingError>,
    component_reports: &'a [&re_viewer_context::VisualizerInstructionReport],
) -> Option<Cow<'a, str>> {
    // Error during mapping. Don't show temporary unavailability as an error.
    let mapping_error_summary = mapping_error
        .filter(|err| !err.is_data_temporarily_unavailable())
        .map(ComponentMappingError::summary);

    // Errors other than component mapping:
    let component_report_error = component_reports
        .iter()
        .find(|report| report.diagnostic.severity == ViewerReportSeverity::Error);

    // Prioritize mapping errors over error reports from the visualizer.
    //
    // Note that these two may overlap:
    // typically when a visualizer hits its first hard mapping error it will stop and report the error.
    // We are however, iterating over *all* mappings here and are interested in all mapping failures,
    // not just the first one the visualizer may have hit.
    mapping_error_summary.map(Cow::Owned).or_else(|| {
        component_report_error.map(|report| Cow::Borrowed(report.diagnostic.summary.as_str()))
    })
}

pub fn raw_default_without_fallback(
    query_result: &crate::BlueprintResolvedLatestAtResults<'_>,
    target_component_descr: &ComponentDescriptor,
) -> Option<Arc<dyn re_chunk::ArrowArray>> {
    let target_component = target_component_descr.component;

    let result_default = query_result.view_defaults.get(target_component)?;
    result_default
        .non_empty_component_batch_raw(target_component)
        .map(|(_, arr)| arr)
}

pub fn raw_default_or_fallback(
    query_ctx: &re_viewer_context::QueryContext<'_>,
    query_result: &crate::BlueprintResolvedLatestAtResults<'_>,
    target_component_descr: &ComponentDescriptor,
) -> Arc<dyn re_chunk::ArrowArray> {
    raw_default_without_fallback(query_result, target_component_descr).unwrap_or_else(|| {
        query_ctx
            .viewer_ctx()
            .component_fallback_registry()
            .fallback_for(target_component_descr, query_ctx)
    })
}

pub fn source_selector_ui(
    ui: &mut egui::Ui,
    label: &str,
    mapping_ctx: &SourceMappingContext<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
    show_default_and_override: bool,
    mapping_error: Option<&ComponentMappingError>,
    component_reports: &[&re_viewer_context::VisualizerInstructionReport],
) {
    let current_selection_error = resolve_current_selection_error(mapping_error, component_reports);

    ui.push_id("source_component", |ui| {
        ui.list_item_flat_noninteractive(list_item::PropertyContent::new(label).value_fn(
            |ui, _| {
                let summary = mapping_ctx.source.summary();
                let selected_text = if current_selection_error.is_some() {
                    egui::RichText::new(&summary).color(ui.tokens().error_fg_color)
                } else {
                    egui::RichText::new(&summary)
                };

                let response = egui::ComboBox::new("source_component_combo_box", "")
                    .selected_text(selected_text)
                    .popup_style(menu_style())
                    .show_ui(ui, |ui| {
                        source_component_items_ui(
                            ui,
                            mapping_ctx,
                            entity_components_with_datatype,
                            query_info,
                            show_default_and_override,
                            mapping_ctx.source,
                            current_selection_error.as_deref(),
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

/// Context for rendering the source component mapping combo box items.
pub struct SourceMappingContext<'a> {
    pub data_result: &'a DataResult,
    pub query_ctx: &'a re_viewer_context::QueryContext<'a>,
    pub target_component_descr: &'a ComponentDescriptor,
    pub is_ui_editable: bool,
    pub instruction: &'a VisualizerInstruction,
    pub source: &'a VisualizerComponentSource,
    pub raw_default: &'a ArrayRef,
    pub annotations: Option<&'a re_viewer_context::Annotations>,
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

fn source_component_items_ui(
    ui: &mut egui::Ui,
    mapping_ctx: &SourceMappingContext<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
    show_default_and_override: bool,
    current: &VisualizerComponentSource,
    current_selection_error: Option<&str>,
) {
    let mut options =
        collect_source_component_options(mapping_ctx, entity_components_with_datatype, query_info);

    if raw_value_for_mapping(
        mapping_ctx,
        mapping_ctx.annotations,
        &VisualizerComponentSource::AnnotationContext,
    )
    .is_some_and(|value| !value.is_empty())
    {
        options.push(VisualizerComponentSource::AnnotationContext);
    }

    let raw_override = mapping_ctx.viewer_ctx().raw_latest_at_in_current_blueprint(
        &mapping_ctx.instruction.override_path,
        mapping_ctx.target_component(),
    );

    if mapping_ctx.is_ui_editable && show_default_and_override {
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
        source_component_item_ui(ui, mapping_ctx, current, current_selection_error, source);
    }

    if show_sections {
        ui.add(re_ui::ComboItemHeader::new("Other values:"));
    }
    for source in &other_options {
        source_component_item_ui(ui, mapping_ctx, current, current_selection_error, source);
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
        && show_default_and_override
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

fn collect_source_component_options(
    mapping_ctx: &SourceMappingContext<'_>,
    entity_components_with_datatype: &[(ComponentIdentifier, DataType)],
    query_info: &VisualizerQueryInfo,
) -> Vec<VisualizerComponentSource> {
    let component_descr = mapping_ctx.target_component_descr;

    let no_mapping_mapping = VisualizerComponentSource::simple_map(component_descr.component);

    let Some(target_component_type) = &component_descr.component_type else {
        return vec![no_mapping_mapping];
    };

    // Collect suitable source components with the same datatype as the target component.

    // TODO(andreas): Right now we are _more_ flexible for required components, because there we also support
    // casting in some special cases. Eventually this should always be the case, leaving us always with a list of valid physical types that we filter on.
    let allowed_physical_types =
        if let re_viewer_context::VisualizabilityConstraints::SingleRequiredComponent(constraint) =
            &query_info.constraints
            && constraint.target_component() == mapping_ctx.target_component()
        {
            constraint.physical_types().clone()
        } else {
            // Get arrow datatype of the target component.
            let reflection = mapping_ctx.viewer_ctx().reflection();
            let Some(target_component_reflection) =
                reflection.components.get(target_component_type)
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

    // Components queried by this visualizer (other than the target) should not appear as
    // source options — they serve a different role in the same visualizer.
    let other_queried: Vec<ComponentIdentifier> = query_info
        .queried
        .iter()
        .map(|d| d.component)
        .filter(|c| *c != component_descr.component)
        .collect();

    entity_components_with_datatype
        .iter()
        .filter(|(source_component, _)| !other_queried.contains(source_component))
        .flat_map(|(source_component, datatype)| {
            use itertools::Either;

            let source_component = *source_component;

            // Direct match?
            if allowed_physical_types.contains(datatype) {
                Either::Left(Either::Left(std::iter::once(
                    VisualizerComponentSource::simple_map(source_component),
                )))
            }
            // Match fields in the struct?
            else if let Some(selectors) = re_lenses_core::extract_nested_fields(datatype, |dt| {
                allowed_physical_types.contains(dt)
            }) {
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

/// Determines which source component options should be in the "Recommended" group.
fn extract_recommended_source_options(
    mapping_ctx: &SourceMappingContext<'_>,
    options: &[VisualizerComponentSource],
) -> Vec<VisualizerComponentSource> {
    // Folks with Rerun access check https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=791-7619&t=6SWixKV9yWMTFQba-0
    // for the original design & rationale.

    let target_component = mapping_ctx.target_component();
    let has_annotation_context = options.contains(&VisualizerComponentSource::AnnotationContext);

    // Rule 1: Identity mapping is recommended.
    if options
        .iter()
        .any(|source| source.is_identity_mapping(target_component))
    {
        let mut recommended = vec![VisualizerComponentSource::identity(target_component)];
        if has_annotation_context {
            recommended.push(VisualizerComponentSource::AnnotationContext);
        }
        return recommended;
    }

    // Rule 2: View-recommended mappings are recommended.
    let view_ctx = mapping_ctx.view_ctx();
    let viewer_ctx = mapping_ctx.viewer_ctx();
    let visualizers_with_reason = visualizers_for_entity(
        viewer_ctx,
        view_ctx.view_class_identifier,
        &mapping_ctx.data_result.entity_path,
    );
    let recommended_visualizers = view_ctx.view_class().recommended_visualizers_for_entity(
        &mapping_ctx.data_result.entity_path,
        &visualizers_with_reason,
        viewer_ctx.indicated_entities_per_visualizer,
    );
    if let Some(recommended_mappings) = recommended_visualizers
        .all_recommendations()
        .get(&mapping_ctx.instruction.visualizer_type)
    {
        let mut recommended: Vec<_> = recommended_mappings
            .iter()
            .filter_map(|mappings| mappings.get_source_for_component(&target_component))
            .filter(|source| options.contains(source))
            .cloned()
            .collect();

        if !recommended.is_empty() {
            if has_annotation_context
                && !recommended.contains(&VisualizerComponentSource::AnnotationContext)
            {
                recommended.push(VisualizerComponentSource::AnnotationContext);
            }
            return recommended;
        }
    }

    // Rule 3: Annotation context is recommended when it resolves a value.
    if has_annotation_context {
        return vec![VisualizerComponentSource::AnnotationContext];
    }

    // Rule 4: Default is recommended if present in the option list & non-empty.
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
    current_selection_error: Option<&str>,
    source: &VisualizerComponentSource,
) {
    let selected = source == current;

    let raw_value = raw_value_for_mapping(mapping_ctx, mapping_ctx.annotations, source);

    let mut item = ComboItem::new(source.summary()).selected(selected);

    if selected && let Some(error) = current_selection_error {
        item = item.error(Some(error.to_owned()));
    } else if let Some(raw_value) = raw_value {
        let num_values = raw_value.len();
        item = item.value_widget(move |ui: &mut Ui| {
            // We intentionally don't show the value if there are multiple values since it can get cluttery. We'll likely iterate on this in the future.
            if num_values > 1 {
                ui.label(format!("{} values", re_format::format_uint(num_values)));
            } else {
                let viewer_ctx = mapping_ctx.viewer_ctx();
                let store_view_ctx = viewer_ctx.active_recording_store_view_context();
                viewer_ctx.component_ui_registry().component_ui_raw(
                    &store_view_ctx,
                    ui,
                    UiLayout::List,
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

pub fn save_component_mapping(
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

/// Extracts the list of visualizers (with their reasons) for a specific entity
/// from the viewer context, without cloning.
pub fn visualizers_for_entity<'a>(
    viewer_ctx: &'a re_viewer_context::ViewerContext<'a>,
    view_class_identifier: ViewClassIdentifier,
    entity_path: &EntityPath,
) -> Vec<(ViewSystemIdentifier, &'a VisualizableReason)> {
    viewer_ctx
        .iter_visualizable_entities_for_view_class(view_class_identifier)
        .filter_map(|(visualizer, ents)| ents.get(entity_path).map(|reason| (visualizer, reason)))
        .collect()
}

pub fn raw_value_for_mapping(
    mapping_ctx: &SourceMappingContext<'_>,
    annotations: Option<&re_viewer_context::Annotations>,
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
            annotations,
            &mapping_ctx.query_ctx.query,
            mapping_ctx.data_result,
            [target_component],
            Some(&hypothetical_instruction),
        );
        query_result.get_raw_cell(target_component)
    }
}
