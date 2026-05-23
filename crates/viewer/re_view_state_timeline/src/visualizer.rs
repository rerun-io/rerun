use nohash_hasher::IntMap;
use re_chunk_store::AbsoluteTimeRange;
use re_chunk_store::external::arrow::datatypes::DataType;
use re_log_types::TimeInt;
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::{StateChange, StateConfiguration};
use re_sdk_types::components::Text;
use re_view::ComponentCastRule;
use re_viewer_context::{
    AppOptions, IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerReportSeverity, VisualizerSystem,
};

use crate::data::{StateLane, StateLanePhase, StateLanesData, StateValueKind};

/// Maps each accepted source physical type to a type that the visualizer can handle.
static COMPONENT_CAST_MAP: std::sync::LazyLock<std::collections::BTreeMap<DataType, DataType>> =
    std::sync::LazyLock::new(|| {
        [
            (DataType::Utf8, DataType::Utf8),
            (DataType::LargeUtf8, DataType::LargeUtf8),
            (DataType::Boolean, DataType::Boolean),
            (DataType::Int8, DataType::Float64),
            (DataType::Int16, DataType::Float64),
            (DataType::Int32, DataType::Float64),
            (DataType::Int64, DataType::Float64),
            (DataType::UInt8, DataType::Float64),
            (DataType::UInt16, DataType::Float64),
            (DataType::UInt32, DataType::Float64),
            (DataType::UInt64, DataType::Float64),
            (DataType::Float16, DataType::Float64),
            (DataType::Float32, DataType::Float64),
            (DataType::Float64, DataType::Float64),
        ]
        .into_iter()
        .collect()
    });

/// Map a post-cast element datatype to its canonical lane kind.
pub fn state_value_kind_from_datatype(dt: &DataType) -> Option<StateValueKind> {
    match dt {
        DataType::Utf8 | DataType::LargeUtf8 => Some(StateValueKind::String),
        DataType::Float64 => Some(StateValueKind::Scalar),
        DataType::Boolean => Some(StateValueKind::Bool),
        _ => None,
    }
}

/// Determine the canonical state value kind for the lane addressed by `instruction`.
pub fn current_state_value_kind(
    ctx: &ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
) -> Option<StateValueKind> {
    let state_component = StateChange::descriptor_state().component;
    let rules: IntMap<_, ComponentCastRule> =
        std::iter::once((state_component, state_cast_rule as ComponentCastRule)).collect();
    let result = re_view::latest_at_with_blueprint_resolved_data_polymorphic(
        ctx,
        None,
        &ctx.current_query(),
        data_result,
        [state_component],
        Some(instruction),
        &rules,
    );
    let arr = result.get_raw_cell(state_component)?;
    state_value_kind_from_datatype(arr.data_type())
}

/// Polymorphic cast rule for the state slot: a thin lookup into [`COMPONENT_CAST_MAP`].
///
/// Returning `None` for an unsupported source type causes the query layer to leave the chunk
/// unchanged (no cast applied). The visualizer then detects this and emits a per-instruction
/// error from `execute()`.
pub fn state_cast_rule(source: &DataType) -> Option<DataType> {
    COMPONENT_CAST_MAP.get(source).cloned()
}

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
        // Accept any of the physical types the polymorphic state cast rule can canonicalize.
        // The source selector consults this set to decide which entity components are offered
        // as candidates for the state slot.
        let constraints =
            SingleRequiredComponentConstraint::new::<Text>(&StateChange::descriptor_state())
                .with_additional_physical_types(COMPONENT_CAST_MAP.keys().cloned())
                .with_allow_static_data(false)
                .into();

        let queried = StateChange::all_components()
            .iter()
            .chain(StateConfiguration::all_components().iter())
            .cloned()
            .collect();

        VisualizerQueryInfo {
            relevant_archetype: StateChange::descriptor_state().archetype,
            constraints,
            queried,
        }
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

        // The state slot is polymorphic on the source datatype: numerics collapse to f64,
        // strings/bools pass through. The post-cast chunks served by the query layer are
        // therefore one of {Utf8, Float64, Boolean}.
        let state_component = StateChange::descriptor_state().component;
        let cast_rules: IntMap<re_sdk_types::ComponentIdentifier, ComponentCastRule> =
            std::iter::once((state_component, state_cast_rule as ComponentCastRule)).collect();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let all_component_ids: Vec<_> = StateChange::all_component_identifiers()
                .chain(StateConfiguration::all_component_identifiers())
                .collect();
            let range_results = re_view::range_with_blueprint_resolved_data_polymorphic(
                ctx,
                None,
                &query,
                data_result,
                all_component_ids,
                instruction,
                &cast_rules,
            );

            let results = re_view::BlueprintResolvedResults::from((query.clone(), range_results));
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let all_values = results.iter_required(state_component);
            if all_values.is_empty() {
                continue;
            }

            // Parse the optional StateConfiguration.
            let state_config = resolve_state_config(&results);

            // Dispatch on the post-cast element type. A null state is a fallthrough, not a
            // phase change: the preceding phase must continue across it. Empty slices represent
            // null entries and are skipped by the per-row filters inside the dispatch.
            let element_types = state_chunk_element_types(&all_values);
            if element_types.len() > 1 {
                let kinds_list = element_types
                    .iter()
                    .map(|dt| format!("{dt:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                results.report_for_component(
                    state_component,
                    VisualizerReportSeverity::Error,
                    format!(
                        "State component type changed over time ({kinds_list}). \
                         The lane cannot be rendered until the column has a single type."
                    ),
                );
                continue;
            }
            let Some(element_type) = element_types.into_iter().next() else {
                continue;
            };
            let Some((value_kind, lane_phases)) =
                lane_phases_for(&all_values, &element_type, &state_config)
            else {
                continue;
            };

            if lane_phases.is_empty() {
                continue;
            }

            // Build the lane label, appending the source component if remapped.
            let lane_label = {
                let base = data_result.entity_path.to_string();
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

            lanes.push(StateLane {
                label: lane_label,
                entity_path: data_result.entity_path.clone(),
                value_kind,
                phases: lane_phases,
            });
        }

        Ok(output.with_visualizer_data(StateLanesData { lanes }))
    }
}

/// Format a typed state value into its lane label string.
///
/// One impl per type the polymorphic state cast can produce. Equality (used for the merge
/// step) is tested on the typed value itself, so this is only invoked once per *surviving*
/// merged phase.
trait StateLabel {
    fn to_lane_label(&self) -> String;
}

impl StateLabel for ArrowString {
    #[inline]
    fn to_lane_label(&self) -> String {
        self.as_str().to_owned()
    }
}

impl StateLabel for f64 {
    #[inline]
    fn to_lane_label(&self) -> String {
        if self.is_finite() && self.fract() == 0.0 && self.abs() < 1e16 {
            // Integer-valued floats: render without a trailing `.0` so config entries typed as
            // `"1"`, `"42"` continue to match values that arrive as `Float64`.
            format!("{}", *self as i64)
        } else {
            format!("{self}")
        }
    }
}

impl StateLabel for bool {
    #[inline]
    fn to_lane_label(&self) -> String {
        if *self { "true" } else { "false" }.to_owned()
    }
}

/// Flatten typed rows into a list of phases.
fn lane_phases_from_rows<T, ChunkIter, RowValues>(
    rows: ChunkIter,
    state_config: &[(String, StateStyle)],
) -> Vec<StateLanePhase>
where
    T: PartialEq + StateLabel,
    ChunkIter: IntoIterator<Item = (TimeInt, RowValues)>,
    RowValues: IntoIterator<Item = T>,
{
    let mut phases: Vec<(i64, T)> = Vec::new();
    for (data_time, row_values) in rows {
        let t = data_time.as_i64();
        for value in row_values {
            if let Some(last) = phases.last_mut()
                && last.0 == t
            {
                last.1 = value;
            } else {
                phases.push((t, value));
            }
        }
    }
    phases.sort_by_key(|(t, _)| *t);
    // `dedup_by(|a, b| ...)` keeps the first of each consecutive run, removing later
    // duplicates — which is exactly the "first phase wins" merge we want.
    phases.dedup_by(|a, b| a.1 == b.1);
    phases
        .into_iter()
        .map(|(t, v)| build_lane_phase(t, &v.to_lane_label(), state_config))
        .collect()
}

/// Look up a formatted phase value in the user-authored `StateConfiguration` and build the
/// corresponding [`StateLanePhase`]. Falls back to a hash-derived color and the raw label when
/// no config entry matches.
fn build_lane_phase(
    start_time: i64,
    formatted: &str,
    state_config: &[(String, StateStyle)],
) -> StateLanePhase {
    if let Some((_, style)) = state_config.iter().find(|(v, _)| v == formatted) {
        StateLanePhase {
            start_time,
            label: style.label.clone(),
            color: style.color,
            visible: style.visible,
        }
    } else {
        StateLanePhase {
            start_time,
            label: formatted.to_owned(),
            color: color_for_value(formatted),
            visible: true,
        }
    }
}

/// Build `StateLanePhase`s for one lane from the polymorphic state slot, alongside the
/// canonical [`StateValueKind`] that drives downstream UI choices.
///
/// Dispatches on the post-cast element type and forwards each typed iterator to
/// [`lane_phases_from_rows`]. Returns `None` for an element type the cast rule rejects (and
/// thus shouldn't ever produce in practice).
fn lane_phases_for(
    all_values: &re_view::HybridResultsChunkIter<'_>,
    element_type: &DataType,
    state_config: &[(String, StateStyle)],
) -> Option<(StateValueKind, Vec<StateLanePhase>)> {
    let kind_and_phases = match element_type {
        DataType::Utf8 | DataType::LargeUtf8 => (
            StateValueKind::String,
            lane_phases_from_rows::<ArrowString, _, _>(
                all_values.slice::<String>().map(|((data_time, _), texts)| {
                    (data_time, texts.into_iter().filter(|s| !s.is_empty()))
                }),
                state_config,
            ),
        ),
        DataType::Float64 => (
            StateValueKind::Scalar,
            lane_phases_from_rows::<f64, _, _>(
                all_values
                    .slice::<f64>()
                    .map(|((data_time, _), values)| (data_time, values.iter().copied())),
                state_config,
            ),
        ),
        DataType::Boolean => (
            StateValueKind::Bool,
            lane_phases_from_rows::<bool, _, _>(
                all_values.slice::<bool>().map(|((data_time, _), values)| {
                    // `BooleanBuffer` only iterates via a borrow on `values`, so materialize a
                    // `Vec<bool>` whose lifetime is detached from this row's stack frame.
                    (data_time, (&values).into_iter().collect::<Vec<bool>>())
                }),
                state_config,
            ),
        ),
        _ => return None,
    };
    Some(kind_and_phases)
}

/// Collect the set of post-cast element types observed across every chunk for the state slot.
///
/// The cast normally produces a single type — one of {`Utf8`, `LargeUtf8`, `Float64`,
/// `Boolean`} — but if the underlying column's physical type changed over time, the chunks
/// come back with mixed element types. Returning the deduped set lets the caller treat
/// "empty", "uniform" and "mixed" by inspecting `len()`.
fn state_chunk_element_types(
    all_values: &re_view::HybridResultsChunkIter<'_>,
) -> std::collections::BTreeSet<DataType> {
    let chunks = all_values.chunks();
    chunks
        .chunks
        .iter()
        .filter_map(|chunk| chunk.components().get_array(chunks.component))
        .map(|arr| arr.value_type())
        .collect()
}
