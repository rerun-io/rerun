use itertools::izip;
use re_chunk_store::AbsoluteTimeRange;
use re_entity_db::EntityPath;
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_query::range_zip_1x2;
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
    /// Unless [`Self::is_full_range`] is set, this only covers [`Self::window`]
    /// (plus all static entries, which any range query returns).
    pub entries: Vec<Entry>,

    /// The time window that was queried, if any.
    pub window: Option<FetchWindow>,

    /// Whether the query covered the entire timeline (used when a log level filter is active).
    pub is_full_range: bool,
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
        let filter_active = state.is_some_and(|state| state.filter_active);
        let window = state
            .and_then(|state| state.fetch_window)
            .filter(|window| window.timeline == view_query.timeline);

        let (time_range, is_full_range) = if filter_active {
            // With a log level filter active we can't derive the row layout from chunk-level
            // metadata (the filter changes the row count), so fall back to fetching everything.
            (AbsoluteTimeRange::EVERYTHING, true)
        } else if let Some(window) = window {
            (AbsoluteTimeRange::new(window.min, window.max), false)
        } else {
            // We don't know the visible window yet (first frame, or the timeline changed).
            // Query a degenerate range: static chunks are returned regardless, and the view
            // will request a repaint once it has computed the window.
            (AbsoluteTimeRange::new(TimeInt::MAX, TimeInt::MAX), false)
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

        Ok(output.with_visualizer_data(TextLogOutput {
            entries,
            window,
            is_full_range,
        }))
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

        let all_levels = results.iter_optional(TextLog::descriptor_level().component);
        let all_colors = results.iter_optional(TextLog::descriptor_color().component);

        let all_frames = range_zip_1x2(
            all_texts.slice::<String>(),
            all_levels.slice::<String>(),
            all_colors.slice::<u32>(),
        );

        let all_frames = izip!(all_timepoints, all_frames);

        for (timepoint, ((data_time, _row_id), bodies, levels, colors)) in all_frames {
            // A text log entry is one row; we only ever look at the first instance of each
            // component. This keeps the row count in sync with the chunk-level metadata that
            // the view uses to lay out the table.
            let Some(body) = bodies.first() else {
                continue;
            };

            entries.push(Entry {
                entity_path: data_result.entity_path.clone(),
                time: data_time,
                timepoint,
                color: colors
                    .and_then(|colors| colors.first().copied())
                    .map(Into::into),
                body: body.clone().into(),
                level: levels
                    .and_then(|levels| levels.first().cloned())
                    .map(Into::into),
            });
        }
    }
}
