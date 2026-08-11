use std::collections::HashMap;

use itertools::izip;
use re_chunk_store::{AbsoluteTimeRange, RowId};
use re_entity_db::EntityPath;
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_view::range_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewStateExt as _,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewProperty;

use crate::row_layout::{LevelCountCache, LevelFilter, RowLayout};
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
/// Written by the view's `ui()` each frame, and read here on the next frame.
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
    /// This only covers [`Self::window`] plus all static entries, and only rows that pass the
    /// level filter.
    pub entries: Vec<Entry>,

    /// The time window that was queried, if any.
    pub window: Option<FetchWindow>,

    /// Where every row of the table lives, including the ones outside [`Self::window`].
    ///
    /// `None` if the visualizer didn't run at all, i.e. the view has no active instructions.
    pub layout: Option<RowLayout>,
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

        // The view knows which time window is actually visible, so we only need query that window.
        let state = ctx.view_state.downcast_ref::<TextViewState>().ok();
        let window = state
            .and_then(|state| state.fetch_window)
            .filter(|window| window.timeline == view_query.timeline);

        let time_range = if let Some(window) = window {
            AbsoluteTimeRange::new(window.min, window.max)
        } else {
            // We don't know the visible window yet, query everything.
            AbsoluteTimeRange::new(TimeInt::MAX, TimeInt::MAX)
        };

        let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range)
            .keep_extra_timelines(true);

        // An *explicit* level filter (i.e. one set in the blueprint, not the show-everything
        // fallback) changes which rows are shown; the row layout then comes from cached
        // per-chunk level counts instead of plain chunk row counts.
        let filter = ViewProperty::from_archetype::<TextLogRows>(ctx)
            .component_array::<TextLogLevel>(
                TextLogRows::descriptor_filter_by_log_level().component,
            )?
            .map(|levels| LevelFilter(levels.iter().map(|lvl| lvl.as_str().to_owned()).collect()));

        // Only the fetched window's entries are materialized; the rest of the row layout comes
        // from chunk-level metadata (time ranges + row counts), so we never have to touch the
        // full data. See <https://github.com/rerun-io/rerun/issues/7562>.
        let layout = {
            let engine = ctx.recording_engine();
            ctx.viewer_ctx
                .store_context
                .memoizer(|level_counts: &mut LevelCountCache| {
                    RowLayout::build(
                        engine.store(),
                        &view_query.timeline,
                        TextLog::descriptor_text().component,
                        TextLog::descriptor_level().component,
                        view_query
                            .iter_visualizer_instruction_for(Self::identifier())
                            .map(|(data_result, _)| &data_result.entity_path),
                        filter.as_ref(),
                        level_counts,
                    )
                })
        };

        if layout.any_missing {
            output.set_missing_chunks();
        }

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

        // The fetched window contains *all* rows in its time range; apply the level filter
        // here so the entries line up with the (filtered) row layout.
        if let Some(filter) = &filter {
            re_tracing::profile_scope!("filter");
            entries.retain(|entry| filter.matches(entry.level.as_ref().map(|lvl| lvl.as_str())));
        }

        {
            // Sort by currently selected timeline.
            re_tracing::profile_scope!("sort");
            entries.sort_by_key(|e| e.time);
        }

        Ok(output.with_visualizer_data(TextLogOutput {
            entries,
            window,
            layout: Some(layout),
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

        // A text log entry is one row, and its level/color are read from that same row only.
        // This is different from the usual latest-at semantics, but keeps the levels and row counts in sync
        // with the chunk-level metadata which we use to layout the table.
        // However, this does mean that if a level/color is overridden in a blueprint, it won't be applied here.
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
                body: bodies.first().cloned().map(Into::into).unwrap_or_default(),
                level: all_levels.get(&index).cloned().map(Into::into),
            });
        }
    }
}
