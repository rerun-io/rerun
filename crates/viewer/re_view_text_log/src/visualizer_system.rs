use std::collections::HashMap;

use itertools::izip;
use re_chunk_store::{AbsoluteTimeRange, RowId};
use re_entity_db::EntityPath;
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_view::range_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewStateExt as _,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use crate::view_class::TextViewState;

#[derive(Debug, Clone)]
pub struct Entry {
    pub entity_path: EntityPath,
    pub time: TimeInt,
    pub timepoint: TimePoint,
    pub color: Option<Color>,
    pub body: Text,
    pub level: Option<TextLogLevel>,
}

/// Time window on a timeline that the visualizer should query.
///
/// Written by the view's `ui()` (which owns the scrolling) each frame, and read here on the
/// *next* frame — the same one-frame feedback loop the time series view uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchWindow {
    pub timeline: TimelineName,
    pub min: TimeInt,
    pub max: TimeInt,
}

impl re_byte_size::SizeBytes for FetchWindow {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

/// Result of executing [`TextLogSystem`] for one frame.
#[derive(Clone, Default)]
pub struct TextLogOutput {
    /// Entries sorted by time on the active timeline, static entries first.
    ///
    /// This only covers [`Self::window`]
    /// (plus all static entries, which any range query returns).
    pub entries: Vec<Entry>,

    /// The time window that was queried, if any.
    pub window: Option<FetchWindow>,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextLogSystem;

impl IdentifiedViewSystem for TextLogSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "TextLog"
        )
    }
}

impl VisualizerSystem for TextLogSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Text>(
            &TextLog::descriptor_text(),
            &TextLog::all_components(),
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

        // The view's `ui()` tells us which time window is actually visible, so that we don't
        // have to query (and materialize entries for) the entire recording.
        // See <https://github.com/rerun-io/rerun/issues/7562>.
        let state = ctx.view_state.downcast_ref::<TextViewState>().ok();
        let window = state
            .and_then(|state| state.fetch_window)
            .filter(|window| window.timeline == view_query.timeline);

        let time_range = if let Some(window) = window {
            AbsoluteTimeRange::new(window.min, window.max)
        } else {
            // We don't know the visible window yet (first frame, or the timeline changed).
            // Query a degenerate range: static chunks are returned regardless, and the view
            // will request a repaint once it has computed the window.
            AbsoluteTimeRange::new(TimeInt::MAX, TimeInt::MAX)
        };

        let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range)
            .keep_extra_timelines(true);

        let mut entries = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            Self::process_visualizer_instruction(
                &mut entries,
                ctx,
                &query,
                data_result,
                instruction,
                &output,
            );
        }

        {
            // Sort by currently selected timeline.
            // The sort is stable, so entries at the same time keep a deterministic order.
            re_tracing::profile_scope!("sort");
            entries.sort_by_key(|e| e.time);
        }

        Ok(output.with_visualizer_data(TextLogOutput { entries, window }))
    }
}

impl TextLogSystem {
    fn process_visualizer_instruction(
        entries: &mut Vec<Entry>,
        ctx: &ViewContext<'_>,
        query: &re_chunk_store::RangeQuery,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
        output: &VisualizerExecutionOutput,
    ) {
        re_tracing::profile_function!();

        let range_results = range_with_blueprint_resolved_data(
            ctx,
            None,
            query,
            data_result,
            TextLog::all_component_identifiers(),
            instruction,
        );

        // Convert to HybridResults for unified access
        let results = re_view::BlueprintResolvedResults::from((query.clone(), range_results));
        let results =
            re_view::VisualizerInstructionQueryResults::new(instruction, &results, output);

        let all_texts = results.iter_required(TextLog::descriptor_text().component);
        if all_texts.is_empty() {
            return;
        }

        let all_timepoints = all_texts
            .chunks()
            .iter()
            .flat_map(|chunk| chunk.iter_component_timepoints());

        // A text log entry is one row, and its level/color are read from that same row only
        // (i.e. the same log call), by joining on the exact `(time, row id)` index — as opposed
        // to the usual latest-at clamping from earlier rows. This keeps the levels in sync with
        // the chunk-level metadata that the view uses to lay out the table (see
        // `ScrollGeometry`); it also means that blueprint overrides and defaults of these
        // components are not applied here.
        let all_levels: HashMap<(TimeInt, RowId), _> = results
            .iter_optional(TextLog::descriptor_level().component)
            .slice::<String>()
            .filter_map(|(index, levels)| levels.first().map(|level| (index, level.clone())))
            .collect();
        let all_colors: HashMap<(TimeInt, RowId), u32> = results
            .iter_optional(TextLog::descriptor_color().component)
            .slice::<u32>()
            .filter_map(|(index, colors)| colors.first().map(|color| (index, *color)))
            .collect();

        let all_frames = izip!(all_timepoints, all_texts.slice::<String>());

        for (timepoint, (index, bodies)) in all_frames {
            let (data_time, _row_id) = index;

            entries.push(Entry {
                entity_path: data_result.entity_path.clone(),
                time: data_time,
                timepoint,
                color: all_colors.get(&index).copied().map(Into::into),
                // We only ever look at the first instance of each component: a text log entry
                // is one row, which keeps the row count in sync with the chunk-level metadata.
                body: bodies.first().cloned().map(Into::into).unwrap_or_default(),
                level: all_levels.get(&index).cloned().map(Into::into),
            });
        }
    }
}
