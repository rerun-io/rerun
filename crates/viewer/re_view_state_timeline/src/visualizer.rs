use nohash_hasher::IntMap;
use re_chunk_store::external::arrow::datatypes::DataType;
use re_chunk_store::{AbsoluteTimeRange, RowId};
use re_log_types::TimeInt;
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::{StateChange, StateConfiguration};
use re_sdk_types::components::Text;
use re_view::{ComponentCastRule, collect_recursive_clears};
use re_viewer_context::{
    AppOptions, IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerReportSeverity, VisualizerSystem,
};

use crate::data::{
    StateLane, StateLanePhase, StateLanePhaseContent, StateLanesData, StateValueKind,
};

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
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "StateVisualizer"
        )
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

        let queried = std::iter::chain(
            StateChange::all_components().iter(),
            StateConfiguration::all_components().iter(),
        )
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

        // Until the view has auto-fit on its first frame, `visible_time_range` is `None`; we
        // query everything so the auto-fit (which runs in `ui`) has the full data to fit to.
        let visible_range = ctx
            .view_state
            .as_any()
            .downcast_ref::<crate::view_class::StateTimelineViewState>()
            .and_then(|state| state.visible_time_range(view_query.timeline))
            .unwrap_or(AbsoluteTimeRange::EVERYTHING);

        // Including extended bounds means we also query the next state right after the visible range.
        // Visually, it doesn't matter, but the hover tooltip needs to show when exactly the state ends.
        let query = re_chunk_store::RangeQuery::new(view_query.timeline, visible_range)
            .include_extended_bounds(true);

        // We get the state (and config) active at the left edge using a latest-at query.
        // The `include_extended_bounds` above only considered visible chunks.
        let window_start_query_time = query.range.min();

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
            let all_component_ids: Vec<_> = std::iter::chain(
                StateChange::all_component_identifiers(),
                StateConfiguration::all_component_identifiers(),
            )
            .collect();

            // In-window data.
            let range_results = re_view::BlueprintResolvedResults::from((
                query.clone(),
                re_view::range_with_blueprint_resolved_data_polymorphic(
                    ctx,
                    None,
                    &query,
                    data_result,
                    all_component_ids.iter().copied(),
                    instruction,
                    &cast_rules,
                ),
            ));
            let range_results = re_view::VisualizerInstructionQueryResults::new(
                instruction,
                &range_results,
                &output,
            );

            // State + config active at the window start.
            let latest_query =
                re_chunk_store::LatestAtQuery::new(query.timeline, window_start_query_time);
            let bootstrap_results = re_view::BlueprintResolvedResults::from((
                latest_query.clone(),
                re_view::latest_at_with_blueprint_resolved_data_polymorphic(
                    ctx,
                    None,
                    &latest_query,
                    data_result,
                    all_component_ids.iter().copied(),
                    Some(instruction),
                    &cast_rules,
                ),
            ));
            let bootstrap_results = re_view::VisualizerInstructionQueryResults::new(
                instruction,
                &bootstrap_results,
                &output,
            );

            let range_values = range_results.iter_required(state_component);
            let bootstrap_values = bootstrap_results.iter_required(state_component);

            // Dispatch on the post-cast element type, observed across both queries. The cast
            // normally yields a single type; a mix means the column's physical type changed.
            let mut element_types = state_chunk_element_types(&range_values);
            element_types.extend(state_chunk_element_types(&bootstrap_values));
            if element_types.len() > 1 {
                let kinds_list = element_types
                    .iter()
                    .map(|dt| format!("{dt:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                range_results.report_for_component(
                    state_component,
                    VisualizerReportSeverity::Error,
                    format!(
                        "State component type changed over time ({kinds_list}). \
                         The lane cannot be rendered until the column has a single type."
                    ),
                );
                continue;
            }
            let element_type = element_types.into_iter().next().or_else(|| {
                // The visible window is panned entirely before the first state change. Probe
                // the entity's state type at the end of time so the lane still renders.
                let latest_query =
                    re_chunk_store::LatestAtQuery::new(view_query.timeline, TimeInt::MAX);
                let probe = re_view::BlueprintResolvedResults::from((
                    latest_query.clone(),
                    re_view::latest_at_with_blueprint_resolved_data_polymorphic(
                        ctx,
                        None,
                        &latest_query,
                        data_result,
                        [state_component],
                        Some(instruction),
                        &cast_rules,
                    ),
                ));
                let probe =
                    re_view::VisualizerInstructionQueryResults::new(instruction, &probe, &output);
                state_chunk_element_types(&probe.iter_required(state_component))
                    .into_iter()
                    .next()
            });
            let Some(element_type) = element_type else {
                continue;
            };
            let Some(value_kind) = state_value_kind_from_datatype(&element_type) else {
                continue;
            };

            // Prefer the in-window `StateConfiguration`; fall back to the bootstrapped one so the
            // colors/labels/visibility stay correct when the config was set before the window.
            let mut state_config = resolve_state_config(&range_results);
            if state_config.is_empty() {
                state_config = resolve_state_config(&bootstrap_results);
            }

            // The bootstrapped state-before-the-window comes first (it has the earliest time),
            // followed by the in-window changes.
            let mut value_events = collect_state_events(&bootstrap_values, &element_type);
            value_events.extend(collect_state_events(&range_values, &element_type));

            // `Clear` archetypes logged on this entity (or on an ancestor with
            // `is_recursive = true`) end the current state regardless of value type.
            let clear_events = collect_recursive_clears(ctx, &query, &data_result.entity_path);

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

            let lane_phases = build_lane_phases(value_events, clear_events, &state_config);

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
/// One impl per type the polymorphic state cast can produce.
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

/// Format a typed iterator of rows into `(time, RowId, Some(label))` events.
fn collect_typed_events<T, ChunkIter, RowValues>(
    rows: ChunkIter,
) -> Vec<(i64, RowId, Option<String>)>
where
    T: StateLabel,
    ChunkIter: IntoIterator<Item = (TimeInt, RowId, RowValues)>,
    RowValues: IntoIterator<Item = T>,
{
    rows.into_iter()
        .flat_map(|(data_time, row_id, row_values)| {
            let t = data_time.as_i64();
            row_values
                .into_iter()
                .map(move |v| (t, row_id, Some(v.to_lane_label())))
        })
        .collect()
}

/// Merge typed value events with `Clear`-derived gap events into a deduplicated phase list.
///
/// Dedup rules:
/// - Same time: later row id wins (last logged event in this time bucket).
/// - Consecutive identical `Some(label)`s collapse to one.
/// - Consecutive `None`s (gaps) collapse to one.
/// - Leading `None`s (no preceding state) are dropped.
fn build_lane_phases(
    value_events: Vec<(i64, RowId, Option<String>)>,
    clear_events: Vec<(TimeInt, RowId)>,
    state_config: &[(String, StateStyle)],
) -> Vec<StateLanePhase> {
    let mut events = value_events;
    events.extend(clear_events.into_iter().map(|(t, r)| (t.as_i64(), r, None)));
    if events.is_empty() {
        return Vec::new();
    }
    events.sort_by_key(|(t, r, _)| (*t, *r));

    let mut phases: Vec<(i64, Option<String>)> = Vec::new();
    for (t, _r, event) in events {
        if let Some(last) = phases.last_mut()
            && last.0 == t
        {
            last.1 = event;
            continue;
        }
        if event.is_none() && phases.last().is_none_or(|(_, last)| last.is_none()) {
            // Leading gap (no preceding state) or gap-after-gap: skip.
            continue;
        }
        if let (Some((_, Some(prev))), Some(next)) = (phases.last(), &event)
            && prev == next
        {
            continue;
        }
        phases.push((t, event));
    }
    if matches!(phases.first(), Some((_, None))) {
        phases.remove(0);
    }

    phases
        .into_iter()
        .map(|(t, event)| StateLanePhase {
            start_time: t,
            content: event.and_then(|label| build_phase_content(&label, state_config)),
        })
        .collect()
}

/// Resolve a formatted phase value against the user-authored `StateConfiguration`.
///
/// Returns `None` (gap) when the matching config entry is hidden; otherwise builds the
/// drawn-phase style. Without a config match, falls back to a hash-derived color and the
/// raw label.
fn build_phase_content(
    label: &str,
    state_config: &[(String, StateStyle)],
) -> Option<StateLanePhaseContent> {
    if let Some((_, style)) = state_config.iter().find(|(v, _)| v == label) {
        style.visible.then(|| StateLanePhaseContent {
            label: style.label.clone(),
            color: style.color,
        })
    } else {
        Some(StateLanePhaseContent {
            color: color_for_value(label),
            label: label.to_owned(),
        })
    }
}

/// Collect typed `(time, RowId, Some(label)/None)` events for one element type from a query
/// result iterator. Returns an empty vec for element types the polymorphic cast can't produce.
fn collect_state_events(
    values: &re_view::HybridResultsChunkIter<'_>,
    element_type: &DataType,
) -> Vec<(i64, RowId, Option<String>)> {
    match element_type {
        DataType::Utf8 | DataType::LargeUtf8 => {
            // `slice::<Option<String>>` preserves null vs empty-string: a null entry is `None`
            // (partial update, no event) while `Some("")` is an explicit reset (gap).
            values
                .slice::<Option<String>>()
                .flat_map(|((data_time, row_id), texts)| {
                    let t = data_time.as_i64();
                    texts.into_iter().filter_map(move |opt| {
                        opt.map(|s| {
                            let event = (!s.is_empty()).then(|| s.to_lane_label());
                            (t, row_id, event)
                        })
                    })
                })
                .collect()
        }
        DataType::Float64 => collect_typed_events::<f64, _, _>(
            values
                .slice::<f64>()
                .map(|((data_time, row_id), values)| (data_time, row_id, values.iter().copied())),
        ),
        DataType::Boolean => collect_typed_events::<bool, _, _>(values.slice::<bool>().map(
            |((data_time, row_id), values)| {
                // `BooleanBuffer` only iterates via a borrow on `values`, so materialize a
                // `Vec<bool>` whose lifetime is detached from this row's stack frame.
                (
                    data_time,
                    row_id,
                    (&values).into_iter().collect::<Vec<bool>>(),
                )
            },
        )),
        _ => Vec::new(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a string config so phase content resolves to a visible drawn phase.
    fn visible_config(values: &[&str]) -> Vec<(String, StateStyle)> {
        values
            .iter()
            .map(|v| {
                (
                    (*v).to_owned(),
                    StateStyle {
                        label: (*v).to_owned(),
                        color: egui::Color32::WHITE,
                        visible: true,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn bootstrapped_state_becomes_leading_phase() {
        // Reproduces RR-4294's pan regression at the data level: the only state change was logged
        // before the visible window (here at its real time t=40, recovered via the bootstrap
        // latest-at), and there are no changes inside the window. The lane must still produce a
        // phase rather than vanishing; rendering clips its off-screen-left start to the edge.
        let cfg = visible_config(&["Idle"]);
        let events = vec![(40, RowId::new(), Some("Idle".to_owned()))];

        let phases = build_lane_phases(events, Vec::new(), &cfg);

        assert_eq!(phases.len(), 1, "{phases:?}");
        assert_eq!(phases[0].start_time, 40, "{phases:?}");
        assert!(phases[0].content.is_some(), "{phases:?}");
    }

    #[test]
    fn in_window_change_at_window_start_wins_over_bootstrap() {
        // If a real change sits at the same time as the bootstrap row, the later row id wins,
        // leaving a single phase with the in-window value.
        let cfg = visible_config(&["Idle", "Moving"]);
        let events = vec![
            (100, RowId::ZERO, Some("Idle".to_owned())), // bootstrap value
            (100, RowId::new(), Some("Moving".to_owned())), // real change at the same time
        ];

        let phases = build_lane_phases(events, Vec::new(), &cfg);

        assert_eq!(phases.len(), 1, "{phases:?}");
        assert_eq!(phases[0].start_time, 100, "{phases:?}");
        assert_eq!(
            phases[0].content.as_ref().map(|c| c.label.as_str()),
            Some("Moving"),
            "{phases:?}"
        );
    }
}
