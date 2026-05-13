use re_chunk_store::AbsoluteTimeRange;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::{StateChange, StateConfiguration};
use re_sdk_types::components::Text;
use re_viewer_context::{
    AppOptions, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::data::{StateLane, StateLanePhase, StateLanesData};

/// Color palette for state change phases.
#[expect(clippy::disallowed_methods)] // These are data-driven visualization colors, not UI theme colors.
const PALETTE: &[egui::Color32] = &[
    egui::Color32::from_rgb(76, 175, 80),   // green
    egui::Color32::from_rgb(255, 183, 77),  // amber
    egui::Color32::from_rgb(66, 165, 245),  // blue
    egui::Color32::from_rgb(239, 83, 80),   // red
    egui::Color32::from_rgb(171, 71, 188),  // purple
    egui::Color32::from_rgb(38, 198, 218),  // teal
    egui::Color32::from_rgb(255, 241, 118), // yellow
    egui::Color32::from_rgb(141, 110, 99),  // brown
];

/// Stable color derived from the raw state value.
///
/// Hashing the value keeps the color fixed as the user adds, reorders, or
/// removes entries in the `StateConfiguration` — unlike an order-based index.
fn color_for_value(value: &str) -> egui::Color32 {
    let hash = re_log_types::hash::Hash64::hash(value).hash64();
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// Resolved configuration for a single state value.
struct StateStyle {
    label: String,
    color: egui::Color32,
    visible: bool,
}

/// Parse a [`StateConfiguration`] from the query results, building a map from raw value to style.
fn resolve_state_config(
    results: &re_view::VisualizerInstructionQueryResults<'_>,
) -> Vec<(String, StateStyle)> {
    let mut config = Vec::new();

    let values_component = StateConfiguration::descriptor_values().component;
    let labels_component = StateConfiguration::descriptor_labels().component;
    let colors_component = StateConfiguration::descriptor_colors().component;
    let visible_component = StateConfiguration::descriptor_visible().component;

    let values: Vec<String> = results
        .iter_optional(values_component)
        .slice::<String>()
        .flat_map(|(_, texts)| texts.into_iter().map(|t| t.to_string()))
        .collect();

    if values.is_empty() {
        return config;
    }

    let labels: Vec<String> = results
        .iter_optional(labels_component)
        .slice::<String>()
        .flat_map(|(_, texts)| texts.into_iter().map(|t| t.to_string()))
        .collect();

    #[expect(clippy::disallowed_methods)] // Data-driven visualization color, not a UI theme color.
    let colors: Vec<egui::Color32> = results
        .iter_optional(colors_component)
        .slice::<u32>()
        .flat_map(|(_, rgba_values)| {
            rgba_values.iter().map(|&rgba| {
                let [r, g, b, a] = rgba.to_be_bytes();
                egui::Color32::from_rgba_unmultiplied(r, g, b, a)
            })
        })
        .collect();

    let visible: Vec<bool> = results
        .iter_optional(visible_component)
        .slice::<bool>()
        .flat_map(|(_, bools)| bools.iter().collect::<Vec<_>>())
        .collect();

    for (i, value) in values.into_iter().enumerate() {
        let label = labels
            .get(i)
            .filter(|l| !l.is_empty())
            .cloned()
            .unwrap_or_else(|| value.clone());
        let color = colors
            .get(i)
            .copied()
            .unwrap_or_else(|| color_for_value(&value));
        let is_visible = visible.get(i).copied().unwrap_or(true);
        config.push((
            value,
            StateStyle {
                label,
                color,
                visible: is_visible,
            },
        ));
    }

    config
}

/// A visualizer that queries [`StateChange`] archetypes and groups them into state change lanes per entity.
///
/// Each entity path becomes one lane. Each distinct state value within a lane gets a unique color.
#[derive(Default)]
pub struct StateVisualizer;

impl IdentifiedViewSystem for StateVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "StateVisualizer".into()
    }
}

impl VisualizerSystem for StateVisualizer {
    fn selection_ui(
        &self,
        ctx: &ViewContext<'_>,
        ui: &mut egui::Ui,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
        type_report: Option<&re_viewer_context::VisualizerTypeReport>,
    ) -> bool {
        // `StateConfiguration.values`/`colors`/`visible` are edited as a group by
        // `state_config_editor` and aren't remappable, so we render source selectors
        // only for the components that are: the primary `StateChange:state` and the
        // optional `StateConfiguration:labels`.
        let selectors = re_selection_panel::SourceSelectorContext::new(
            ctx,
            data_result,
            instruction,
            self,
            type_report,
        );
        // For state values, default and override options aren't meaningful.
        selectors.render(ui, &StateChange::descriptor_state(), false);
        selectors.render(ui, &StateConfiguration::descriptor_labels(), true);

        crate::visualizer_ui::state_config_editor(ui, ctx, data_result, instruction);
        true
    }

    fn visualizer_query_info(&self, _app_options: &AppOptions) -> VisualizerQueryInfo {
        let all_components: Vec<_> = StateChange::all_components()
            .iter()
            .chain(StateConfiguration::all_components().iter())
            .cloned()
            .collect();
        VisualizerQueryInfo::single_required_component::<Text>(
            &StateChange::descriptor_state(),
            &all_components,
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let query =
            re_chunk_store::RangeQuery::new(view_query.timeline, AbsoluteTimeRange::EVERYTHING);

        let mut lanes: Vec<StateLane> = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let all_component_ids: Vec<_> = StateChange::all_component_identifiers()
                .chain(StateConfiguration::all_component_identifiers())
                .collect();
            let range_results = re_view::range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                all_component_ids,
                instruction,
            );

            let results = re_view::BlueprintResolvedResults::from((query.clone(), range_results));
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let all_texts = results.iter_required(StateChange::descriptor_state().component);
            if all_texts.is_empty() {
                continue;
            }

            // Parse the optional StateConfiguration.
            let state_config = resolve_state_config(&results);

            // Collect (time, text) pairs.
            // A null state is a fallthrough, not a phase change: the preceding phase
            // must continue across it. `slice::<String>` represents null entries as
            // zero-length slices, so we skip empty texts here.
            // TODO(aedm): use string refs while collecting
            let mut phases: Vec<(i64, String)> = Vec::new();
            for ((data_time, _row_id), texts) in all_texts.slice::<String>() {
                let time_value = data_time.as_i64();
                for text in texts {
                    if text.is_empty() {
                        continue;
                    }
                    // If the start of this phase equals the start of the last phase, then just overwrite it.
                    if let Some(last) = phases.last_mut()
                        && last.0 == time_value
                    {
                        last.1 = text.to_string();
                        continue;
                    }
                    phases.push((time_value, text.to_string()));
                }
            }

            if phases.is_empty() {
                continue;
            }

            phases.sort_by_key(|(t, _)| *t);

            // Build the lane label, appending the source component if remapped.
            let lane_label = {
                let base = data_result.entity_path.to_string();
                let state_component = StateChange::descriptor_state().component;
                match instruction.component_mappings.get(&state_component) {
                    Some(re_viewer_context::VisualizerComponentSource::SourceComponent {
                        source_component,
                        ..
                    }) if source_component != &state_component => {
                        format!("{base} ({source_component})")
                    }
                    _ => base,
                }
            };

            // Build the lane. If a value appears in the config, use its label/color/visibility;
            // otherwise derive a stable color from the value itself.
            let lane = StateLane {
                label: lane_label,
                entity_path: data_result.entity_path.clone(),
                phases: phases
                    .into_iter()
                    .map(|(t, value)| {
                        if let Some((_, style)) = state_config.iter().find(|(v, _)| v == &value) {
                            StateLanePhase {
                                start_time: t,
                                label: style.label.clone(),
                                color: style.color,
                                visible: style.visible,
                            }
                        } else {
                            let color = color_for_value(&value);
                            StateLanePhase {
                                start_time: t,
                                label: value,
                                color,
                                visible: true,
                            }
                        }
                    })
                    .collect(),
            };
            lanes.push(lane);
        }

        Ok(output.with_visualizer_data(StateLanesData { lanes }))
    }
}
