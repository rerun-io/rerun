//! Turns logged `StateChange` rows into the colored lanes the state timeline view draws.
//!
//! Every frame, for each visualizer instruction (≈ one per entity):
//!
//! ```text
//!        visible time range (blueprint)  ∩  pan/zoom window (view state)
//!                                    │
//!                 ┌──────────────────┴──────────────────┐
//!                 │             chunk store             │
//!                 ├──────────────────┬──────────────────┤
//!                 │  range query     │  latest-at at    │
//!                 │  over the window │  the window start│
//!                 │                  │                  │
//!                 │  + extended      │  "bootstrap":    │
//!                 │    bounds, so    │  the state that  │
//!                 │    the last      │  was already     │
//!                 │    phase knows   │  active when the │
//!                 │    where it ends │  window opened   │
//!                 └──────────────────┴──────────────────┘
//!                                    │
//!                                    │ polymorphic cast: every source
//!                                    │ type we accept collapses to a
//!                                    │ string, an f64 or a bool
//!                                    ▼
//!                     StateRow { time, row_id, one label per instance }
//!                                    │
//!    Clear archetypes ───────────────┤
//!    (end the current state)         │
//!                                    ▼
//!                            build_lane_phases
//!                            · later row id wins within a time
//!                            · identical neighbors collapse
//!                            · a reset opens a gap
//!                                    │
//!    StateConfiguration ─────────────┤
//!    (label, color, visible)         │ a hidden value becomes a gap;
//!                                    │ an unconfigured one gets a
//!                                    ▼ hash-derived color
//!                              StateLanePhase
//!                                    │
//!                                    ▼
//!                                StateLane                one per instance of the state array
//!                                    │
//!                                    ▼
//!                             StateLaneGroup              one per instruction; its lanes
//!                                    │                    share a label and a configuration
//!                                    ▼
//!                            StateLanesOutput             what this module hands to the view
//!                                    │
//!                                    ▼
//!                             view_class::ui              pans, zooms, paints the bands
//! ```
//!
//! Phases carry only a start time: each one runs until the next begins, and the last one is
//! open-ended. Where that end lands on screen is the view's business, not this module's, see
//! [`crate::StateLanePhase`].

use nohash_hasher::IntMap;
use re_chunk_store::external::arrow::datatypes::DataType;
use re_chunk_store::{AbsoluteTimeRange, RowId};
use re_log_types::TimeInt;
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::{StateChange, StateConfiguration};
use re_sdk_types::blueprint::archetypes::TimeAxis;
use re_sdk_types::blueprint::components::LinkAxis;
use re_sdk_types::components::Text;
use re_view::{ComponentCastRule, collect_recursive_clears};
use re_viewer_context::{
    AppOptions, IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    ViewerReportSeverity, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewProperty;

use crate::data::{
    StateLane, StateLaneGroup, StateLanePhase, StateLanePhaseContent, StateLanesOutput,
    StateValueKind,
};

/// One logged row of the state component.
struct StateRow {
    time: i64,
    row_id: RowId,

    /// One formatted label per instance in the row's state array.
    labels: Vec<Option<String>>,
}

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

/// A visualizer that queries [`StateChange`] archetypes and groups them into state change lanes.
///
/// Each visualizer instruction (typically one per entity path) becomes one lane group, with one
/// lane per state instance. Each distinct state value within a lane gets a unique color.
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

        // The pan/zoom window the view is about to draw — see `view_class::view_window`. Deriving it
        // here rather than reading back what was drawn last frame keeps the query in step with a
        // window that follows the time cursor, and keeps the first frame of a long recording from
        // querying all of it.
        //
        // Without a time range for the timeline we have nothing to derive it from — and no data to
        // speak of either, so query everything.
        let view_window = if let Some(timeline_range) = ctx
            .viewer_ctx
            .recording()
            .time_range_for(&view_query.timeline)
            && let Some(state) = ctx
                .view_state
                .as_any()
                .downcast_ref::<crate::view_class::StateTimelineViewState>()
        {
            let time_axis = ViewProperty::from_archetype::<TimeAxis>(ctx);
            let link = time_axis
                .component_or_fallback::<LinkAxis>(ctx, TimeAxis::descriptor_link().component)?;

            let (_, time_view) = crate::view_class::view_window(
                ctx.viewer_ctx,
                state,
                link,
                view_query.timeline,
                Some(timeline_range),
                view_query.latest_at,
                crate::view_class::data_time_range_of(timeline_range),
            );
            crate::view_class::window_time_range(time_view)
        } else {
            AbsoluteTimeRange::EVERYTHING
        };

        let builder = LaneGroupBuilder::new(ctx, view_query, &output);

        let mut groups: Vec<StateLaneGroup> = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let visible_time_range = match data_result.query_range() {
                re_viewer_context::QueryRange::TimeRange(time_range) => {
                    re_view::resolve_visible_time_range(ctx.viewer_ctx, time_range)
                }

                // Safety: our `default_query_range` is a time range and the selection panel refuses to
                // store latest-at as an override, so this shouldn't happen.
                re_viewer_context::QueryRange::LatestAt => AbsoluteTimeRange::EVERYTHING,
            };

            // The on-screen window, cut down to the configured range.
            let store_range = view_window.intersection(visible_time_range);

            let Some(source) = builder.lane_source(data_result, instruction, store_range) else {
                continue;
            };

            let value_kind = match source
                .element_type
                .as_ref()
                .map(state_value_kind_from_datatype)
            {
                Some(Some(value_kind)) => Some(value_kind),

                // A type the cast cannot turn into a lane: nothing to render.
                // TODO(RR-5426): show the lane and report an error.
                Some(None) => continue,

                // An entity with no state data at all on this timeline still gets an empty
                // lane, and then there is no value kind to report for it.
                None => None,
            };

            groups.push(build_group(
                data_result,
                instruction,
                visible_time_range,
                value_kind,
                &source,
            ));
        }

        Ok(output.with_visualizer_data(StateLanesOutput { groups }))
    }
}

/// Everything one visualizer instruction contributes to its lane group, as pulled from the store.
struct LaneSource {
    /// Post-cast element type of the state column, shared by all its lanes.
    ///
    /// `None` when the entity has no state data at all on this timeline — the group is still shown,
    /// as a single empty lane.
    element_type: Option<DataType>,

    /// How many lanes the group has, i.e. the length of the state arrays.
    instance_count: usize,

    /// State changes, earliest first: the state active before the window, then the in-window ones.
    rows: Vec<StateRow>,

    /// User-authored styling for the state values, keyed by raw value.
    state_config: Vec<(String, StateStyle)>,

    /// `Clear`s that end whatever state is active when they land.
    clear_events: Vec<(TimeInt, RowId)>,
}

/// Builds the lane groups of one [`StateVisualizer::execute`], holding what they all share.
///
/// One group is built per visualizer instruction, in two steps: [`Self::lane_source`] gathers the
/// data from the store, [`build_group`] turns it into lanes.
struct LaneGroupBuilder<'a> {
    ctx: &'a ViewContext<'a>,
    view_query: &'a ViewQuery<'a>,
    output: &'a VisualizerExecutionOutput,

    /// The state slot is polymorphic on the source datatype: numerics collapse to f64,
    /// strings/bools pass through. The post-cast chunks served by the query layer are
    /// therefore one of {Utf8, Float64, Boolean}.
    cast_rules: IntMap<re_sdk_types::ComponentIdentifier, ComponentCastRule>,

    /// Everything we read per lane, across both archetypes.
    queried_components: Vec<re_sdk_types::ComponentIdentifier>,
}

impl<'a> LaneGroupBuilder<'a> {
    fn new(
        ctx: &'a ViewContext<'a>,
        view_query: &'a ViewQuery<'a>,
        output: &'a VisualizerExecutionOutput,
    ) -> Self {
        Self {
            ctx,
            view_query,
            output,
            cast_rules: std::iter::once((
                StateChange::descriptor_state().component,
                state_cast_rule as ComponentCastRule,
            ))
            .collect(),
            queried_components: std::iter::chain(
                StateChange::all_component_identifiers(),
                StateConfiguration::all_component_identifiers(),
            )
            .collect(),
        }
    }

    /// Query the state changes, styling and `Clear`s for one instruction.
    fn lane_source(
        &self,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
        store_range: Option<AbsoluteTimeRange>,
    ) -> Option<LaneSource> {
        let state_component = StateChange::descriptor_state().component;

        // The window can be entirely off the data (panned before or after it, or cut away by the
        // configured visible range), in which case no query says anything about the lane. The shape
        // probe reads it straight from the store, ignoring both the window and `Clear`s — lanes
        // vanishing is what leaves the view with nothing to pan back with.
        let probed_shape = std::cell::OnceCell::new();
        let probe = || {
            probed_shape
                .get_or_init(|| {
                    probe_state_shape(
                        self.ctx,
                        self.view_query.timeline,
                        &data_result.entity_path,
                        instruction,
                        state_component,
                    )
                })
                .as_ref()
        };

        let Some(store_range) = store_range else {
            return Some(LaneSource {
                element_type: probe().map(|shape| shape.value_type.clone()),
                instance_count: probe().map_or(1, |shape| shape.instance_count),
                rows: Vec::new(),
                state_config: Vec::new(),
                clear_events: Vec::new(),
            });
        };

        // Including extended bounds means we also query the next state right after the
        // window. Visually, it doesn't matter, but the hover tooltip needs to show when
        // exactly the state ends.
        let query = re_chunk_store::RangeQuery::new(self.view_query.timeline, store_range)
            .include_extended_bounds(true);

        // In-window data.
        let range_results = re_view::BlueprintResolvedResults::from((
            query.clone(),
            re_view::range_with_blueprint_resolved_data_polymorphic(
                self.ctx,
                None,
                &query,
                data_result,
                self.queried_components.iter().copied(),
                instruction,
                &self.cast_rules,
            ),
        ));
        let range_results = re_view::VisualizerInstructionQueryResults::new(
            instruction,
            &range_results,
            self.output,
        );

        // State + config active at the left edge, which we get with a latest-at query: the
        // `include_extended_bounds` above only considered visible chunks.
        let latest_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.range.min());
        let bootstrap_results = re_view::BlueprintResolvedResults::from((
            latest_query.clone(),
            re_view::latest_at_with_blueprint_resolved_data_polymorphic(
                self.ctx,
                None,
                &latest_query,
                data_result,
                self.queried_components.iter().copied(),
                Some(instruction),
                &self.cast_rules,
            ),
        ));
        let bootstrap_results = re_view::VisualizerInstructionQueryResults::new(
            instruction,
            &bootstrap_results,
            self.output,
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
                ViewerReportSeverity::Error,
                format!(
                    "State component type changed over time ({kinds_list}). \
                     The lane cannot be rendered until the column has a single type."
                ),
            );
            return None;
        }
        let element_type = element_types
            .into_iter()
            .next()
            .or_else(|| probe().map(|shape| shape.value_type.clone()));

        // Prefer the in-window `StateConfiguration`; fall back to the bootstrapped one so the
        // colors/labels/visibility stay correct when the config was set before the window.
        let mut state_config = resolve_state_config(&range_results);
        if state_config.is_empty() {
            state_config = resolve_state_config(&bootstrap_results);
        }

        // The bootstrapped state-before-the-window comes first (it has the earliest time),
        // followed by the in-window changes.
        let mut rows = Vec::new();
        if let Some(element_type) = &element_type {
            rows = collect_state_rows(&bootstrap_values, element_type);
            rows.extend(collect_state_rows(&range_values, element_type));
        }

        // With no rows on screen, fall back to the shape probed from the store, and finally to a
        // single empty lane.
        let instance_count = rows
            .iter()
            .map(|row| row.labels.len())
            .max()
            .or_else(|| probe().map(|shape| shape.instance_count))
            .unwrap_or(1);

        Some(LaneSource {
            element_type,
            instance_count,
            rows,
            state_config,
            // `Clear` archetypes logged on this entity (or on an ancestor with
            // `is_recursive = true`) end the current state regardless of value type.
            clear_events: collect_recursive_clears(self.ctx, &query, &data_result.entity_path),
        })
    }
}

/// Assemble one lane group: a lane per state instance, under a shared label.
fn build_group(
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
    visible_time_range: AbsoluteTimeRange,
    value_kind: Option<StateValueKind>,
    source: &LaneSource,
) -> StateLaneGroup {
    let instance_count = source.instance_count;

    let lanes = (0..instance_count)
        .map(|instance| StateLane {
            phases: build_lane_phases(
                source
                    .rows
                    .iter()
                    .map(|row| {
                        (
                            row.time,
                            row.row_id,
                            row.labels.get(instance).cloned().flatten(),
                        )
                    })
                    .collect(),
                &source.clear_events,
                &source.state_config,
            ),
        })
        .collect();

    StateLaneGroup {
        label: lane_group_label(data_result, instruction, instance_count),
        visible_time_range,
        entity_path: data_result.entity_path.clone(),
        value_kind,
        lanes,
    }
}

/// A lane group's display label: the entity path, plus the source component when the state slot is
/// remapped, plus a `[]` suffix marking a multi-instance group.
fn lane_group_label(
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
    instance_count: usize,
) -> String {
    let state_component = StateChange::descriptor_state().component;

    let mut label = data_result.entity_path.to_string();
    if let Some(re_viewer_context::VisualizerComponentSource::SourceComponent {
        source_component,
        ..
    }) = instruction.component_mappings.get(&state_component)
        && source_component != &state_component
    {
        label = format!("{label} ({source_component})");
    }
    if instance_count > 1 {
        label.push_str("[]");
    }
    label
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

/// Format a typed iterator of rows into [`StateRow`]s.
fn collect_typed_rows<T, ChunkIter, RowValues>(rows: ChunkIter) -> Vec<StateRow>
where
    T: StateLabel,
    ChunkIter: IntoIterator<Item = (TimeInt, RowId, RowValues)>,
    RowValues: IntoIterator<Item = Option<T>>,
{
    rows.into_iter()
        .map(|(data_time, row_id, row_values)| StateRow {
            time: data_time.as_i64(),
            row_id,
            labels: row_values
                .into_iter()
                .map(|v| v.map(|v| v.to_lane_label()))
                .collect(),
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
    clear_events: &[(TimeInt, RowId)],
    state_config: &[(String, StateStyle)],
) -> Vec<StateLanePhase> {
    let mut events = value_events;
    events.extend(clear_events.iter().map(|&(t, r)| (t.as_i64(), r, None)));
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

/// Collect typed state rows for one element type from a query result iterator.
/// Returns no rows for element types the polymorphic cast can't produce.
///
/// Null values, empty strings, and empty arrays all reset (see [`StateRow`]); any other
/// value starts a new phase for its instance.
fn collect_state_rows(
    values: &re_view::HybridResultsChunkIter<'_>,
    element_type: &DataType,
) -> Vec<StateRow> {
    match element_type {
        DataType::Utf8 | DataType::LargeUtf8 => {
            // Strings get their own path: unlike the typed collector, an empty string is
            // also a reset for its instance.
            values
                .slice::<Option<String>>()
                .map(|((data_time, row_id), texts)| StateRow {
                    time: data_time.as_i64(),
                    row_id,
                    labels: texts
                        .into_iter()
                        .map(|opt| opt.filter(|s| !s.is_empty()).map(|s| s.to_lane_label()))
                        .collect(),
                })
                .collect()
        }
        DataType::Float64 => collect_typed_rows::<f64, _, _>(
            values
                .slice::<Option<f64>>()
                .map(|((data_time, row_id), values)| (data_time, row_id, values)),
        ),
        DataType::Boolean => collect_typed_rows::<bool, _, _>(
            values
                .slice::<Option<bool>>()
                .map(|((data_time, row_id), values)| (data_time, row_id, values)),
        ),
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

/// The shape of the state data logged for a lane, read straight from the store. Used as a fallback
/// when no rows are on screen.
struct ProbedStateShape {
    /// Post-cast element type of the state values, as [`state_chunk_element_types`] reports it.
    value_type: DataType,

    /// Length of the state arrays, i.e. how many lanes the group has.
    instance_count: usize,
}

/// Probe the shape of the state data logged for `entity_path` on `timeline`. Ignores the visible
/// window and `Clear`s.
fn probe_state_shape(
    ctx: &ViewContext<'_>,
    timeline: re_log_types::TimelineName,
    entity_path: &re_log_types::EntityPath,
    instruction: &re_viewer_context::VisualizerInstruction,
    state_component: re_sdk_types::ComponentIdentifier,
) -> Option<ProbedStateShape> {
    use re_chunk_store::external::arrow::array::Array as _;

    // Component remappings redirect the state slot to another source component.
    let source_component = match instruction.component_mappings.get(&state_component) {
        Some(re_viewer_context::VisualizerComponentSource::SourceComponent {
            source_component,
            ..
        }) => *source_component,
        _ => state_component,
    };

    let full_range_query = re_chunk_store::RangeQuery::new(timeline, AbsoluteTimeRange::EVERYTHING);
    let engine = ctx.recording_engine();
    let results = engine.store().range_relevant_chunks(
        re_chunk_store::ChunkTrackingMode::Ignore,
        &full_range_query,
        entity_path,
        source_component,
    );

    // Best effort: check the first piece of data we find instead of scanning the entire timeline.
    for chunk in &results.chunks {
        if let Some(array) = chunk.components().get_array(source_component) {
            for i in 0..array.len() {
                if array.is_valid(i) {
                    let value_type = array.value_type();
                    return Some(ProbedStateShape {
                        value_type: state_cast_rule(&value_type).unwrap_or(value_type),
                        instance_count: (array.value_length(i) as usize).max(1),
                    });
                }
            }
        }
    }

    None
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

        let phases = build_lane_phases(events, &[], &cfg);

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

        let phases = build_lane_phases(events, &[], &cfg);

        assert_eq!(phases.len(), 1, "{phases:?}");
        assert_eq!(phases[0].start_time, 100, "{phases:?}");
        assert_eq!(
            phases[0].content.as_ref().map(|c| c.label.as_str()),
            Some("Moving"),
            "{phases:?}"
        );
    }
}
