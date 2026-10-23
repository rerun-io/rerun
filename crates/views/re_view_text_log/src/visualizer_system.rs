use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use re_chunk_store::{AbsoluteTimeRange, RangeQuery};
use re_log_types::{EntityPath, TimeInt, TimePoint, TimelineName};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_sdk_types::{Archetype as _, ArrowString, ComponentIdentifier};
use re_view::{BlueprintResolvedResults, VisualizerInstructionQueryResults};
use re_viewer_context::{
    DataResult, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewStateExt as _, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerInstruction,
    VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewProperty;

use crate::text_log_index_cache::{IndexedRow, TextLogIndex, TextLogIndexCache, TextLogQueryKey};
use crate::text_log_level_cache::TextLogLevelCache;
use crate::text_log_navigation::TextLogNavigation;
use crate::text_log_query::TextLogQuery;
use crate::text_log_source::TextLogSourceKey;
use crate::text_log_span_index::TextLogSpanIndex;
use crate::view_class::TextViewState;

/// Display data for one requested text instance, including times on every retained timeline.
#[derive(Debug, Clone, re_byte_size::SizeBytes)]
pub struct Entry {
    pub entity_path: EntityPath,
    pub timepoint: TimePoint,
    pub color: Option<Color>,
    pub body: ArrowString,
    pub level: Option<ArrowString>,
}

/// Progressive navigation and viewport-only display data.
#[derive(Default, re_byte_size::SizeBytes)]
pub struct TextLogEntries {
    /// Row ordering and refinement readiness.
    pub navigation: TextLogNavigation,

    /// Table row of `entries[0]`, when `entries` is nonempty.
    pub first_entry: usize,

    /// Contiguous display slots starting at `first_entry`; unresolved or unavailable rows are empty.
    pub entries: Vec<Option<Entry>>,

    /// Unfiltered level names accumulated from queried spans for this configuration.
    pub levels: Arc<BTreeSet<String>>,
}

impl TextLogEntries {
    /// Display data for a global table row, if materialized in this frame's window.
    pub fn entry(&self, row: usize) -> Option<&Entry> {
        row.checked_sub(self.first_entry)
            .and_then(|row| self.entries.get(row))
            .and_then(Option::as_ref)
    }
}

/// Source ordering must match cached row identities, including instructions without text.
pub struct TextLogSource<'a> {
    pub data_result: &'a DataResult,
    pub instruction: &'a VisualizerInstruction,
    timeline: TimelineName,
}

impl<'a> TextLogSource<'a> {
    /// Also returns whether all chunks the query touched were loaded.
    /// If not, the results are partial and the missing chunks have been requested.
    fn query_range(
        &self,
        ctx: &'a ViewContext<'a>,
        range: AbsoluteTimeRange,
        components: &[ComponentIdentifier],
    ) -> (BlueprintResolvedResults<'a>, bool) {
        let carry_forward = [
            TextLog::descriptor_level().component,
            TextLog::descriptor_color().component,
        ]
        .into_iter()
        .filter(|component| components.contains(component))
        .collect::<Vec<_>>();
        let query = RangeQuery::new(self.timeline, range).keep_extra_timelines(true);
        let mut results = re_view::range_with_blueprint_resolved_data(
            ctx,
            None,
            &query,
            self.data_result,
            components.iter().copied(),
            self.instruction,
        );
        // Capture fetch readiness before merging, which can hide pending payloads.
        let mut chunks_loaded = !results.any_missing_chunks();
        if !carry_forward.is_empty() {
            // Updates at the window start stay in the range to preserve row-ID ordering.
            let predecessor = if range.min() == TimeInt::MIN {
                re_chunk_store::LatestAtQuery::new_static()
            } else {
                re_chunk_store::LatestAtQuery::new(self.timeline, range.min().dec())
            };
            let bootstrap = re_view::latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &predecessor,
                self.data_result,
                carry_forward,
                Some(self.instruction),
            );
            chunks_loaded &= !bootstrap.any_missing_chunks();
            results.merge_bootstrapped_data(bootstrap);
        }
        ((query, results).into(), chunks_loaded)
    }

    fn populate(
        &self,
        ctx: &'a ViewContext<'a>,
        source: usize,
        rows: &[IndexedRow],
        entries: &mut [Option<Entry>],
        output: &VisualizerExecutionOutput,
        discovered: &mut BTreeSet<String>,
    ) {
        let mut times = rows
            .iter()
            .filter(|row| row.source == source)
            .map(|row| row.time);
        let Some(min) = times.next() else {
            return;
        };
        let max = times.next_back().unwrap_or(min);
        let range = AbsoluteTimeRange::new(min, max);

        let text = TextLog::descriptor_text().component;
        let level = TextLog::descriptor_level().component;
        let color = TextLog::descriptor_color().component;

        let (results, _) = self.query_range(ctx, range, &[text, level, color]);
        let results = VisualizerInstructionQueryResults::new(self.instruction, &results, output);
        let texts = results.iter_required(text);
        let levels = results.iter_optional(level);
        let colors = results.iter_optional(color);
        TextLogQuery {
            entity_path: &self.data_result.entity_path,
            timeline: self.timeline,
            texts: texts.chunks(),
            levels: levels.chunks(),
            colors: colors.chunks(),
        }
        .populate(source, rows, entries, discovered);
    }
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

    /// Resolves navigation metadata and queries display data around the viewport.
    ///
    /// Both metadata sources refine estimated spans near the viewport or pending cursor seek.
    /// Caches retain only navigation metadata and owned labels, never recording payloads.
    ///
    /// Each instruction queries display components over the selected rows' time range with
    /// overscan and optional-value bootstrapping. Only selected identities are materialized;
    /// missing text leaves empty slots, while optional values update as their chunks arrive.
    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        // Absence means unfiltered. The UI fallback only lists discovered labels, which may
        // be incomplete during progressive discovery and must not restrict further discovery.
        let filter = ViewProperty::from_archetype::<TextLogRows>(ctx)
            .component_array::<TextLogLevel>(
                TextLogRows::descriptor_filter_by_log_level().component,
            )?
            .map(|levels| {
                levels
                    .iter()
                    .map(|lvl| lvl.0.0.clone())
                    .collect::<BTreeSet<_>>()
            });

        let sources: Vec<_> = view_query
            .iter_visualizer_instruction_for(Self::identifier())
            .map(|(data_result, instruction)| TextLogSource {
                data_result,
                instruction,
                timeline: view_query.timeline,
            })
            .collect();
        let entries =
            Self::execute_progressive(ctx, view_query, &sources, filter.as_ref(), &output)?;

        Ok(output.with_visualizer_data(Arc::new(entries)))
    }
}

impl TextLogSystem {
    fn execute_progressive(
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        sources: &[TextLogSource<'_>],
        filter: Option<&BTreeSet<ArrowString>>,
        output: &VisualizerExecutionOutput,
    ) -> Result<TextLogEntries, ViewSystemExecutionError> {
        // Identify the shared navigation index: views with the same timeline, filter and instruction mappings reuse one index.
        let key = TextLogQueryKey::new(view_query.timeline, filter, sources);
        let source_keys: Vec<_> = sources
            .iter()
            .map(|source| TextLogSourceKey {
                entity: source.data_result.entity_path.clone(),
                timeline: source.timeline,
                text: source
                    .instruction
                    .source_component(TextLog::descriptor_text().component),
                level: source
                    .instruction
                    .source_component(TextLog::descriptor_level().component),
            })
            .collect();

        // Snapshot the index with pending store changes applied, then plan this view's
        // viewport and protect its rows from eviction by other views sharing the index.
        let cached = ctx
            .viewer_ctx
            .store_context
            .memoizer(|cache: &mut TextLogIndexCache| {
                cache.entry(ctx.recording(), key, &source_keys)
            });
        let mut index = cached.index.clone();
        let state = ctx.view_state.downcast_ref::<TextViewState>()?;
        let plan = state
            .navigation
            .plan(key, cached.layout, view_query.latest_at, &index);
        ctx.viewer_ctx
            .store_context
            .memoizer(|cache: &mut TextLogIndexCache| {
                cache.protect(
                    key,
                    ctx.view_id,
                    index.protected_range(plan.protected_rows.clone()),
                );
            });

        let text = TextLog::descriptor_text().component;
        let level = TextLog::descriptor_level().component;
        let mut discovered_levels = BTreeSet::new();

        // Resolve at most one estimated span per frame into exact rows, so large recordings
        // load progressively instead of querying everything up front.
        if let Some((span_id, range)) = plan.next_span {
            let mut all_chunks_loaded = true;
            let mut exact_index = TextLogIndex::default();
            for (source_id, (source, source_key)) in
                std::iter::zip(sources, &source_keys).enumerate()
            {
                // Sources without text in the span contribute no rows, but querying them would
                // still bootstrap carried-forward level/color, requesting unrelated payloads and
                // keeping the span unpublished until they arrive.
                // The temporary storage read guard must drop before `query_range` reacquires it.
                let has_text = !source_key
                    .text_ranges(
                        ctx.recording().rrd_manifest_index().manifest(),
                        ctx.recording().storage_engine().store(),
                        range,
                    )
                    .is_empty();
                if !has_text {
                    continue;
                }
                let (results, source_chunks_loaded) =
                    source.query_range(ctx, range, &[text, level]);
                all_chunks_loaded &= source_chunks_loaded;
                let results =
                    VisualizerInstructionQueryResults::new(source.instruction, &results, output);
                let texts = results.iter_required(text);
                let levels = results.iter_optional(level);
                exact_index.append(
                    source_id,
                    view_query.timeline,
                    filter,
                    texts.chunks(),
                    levels.chunks(),
                    &mut discovered_levels,
                );
            }
            // Bootstrapped optional values may be mapped from the text component itself.
            exact_index.rows.retain(|row| range.contains(row.time));
            exact_index.rows.sort_by_key(|row| row.time);
            // Publish only fully loaded spans; `refine` also rejects them if the layout
            // changed while querying. Otherwise the span is retried on a later frame.
            if all_chunks_loaded
                && let Some(updated) =
                    ctx.viewer_ctx
                        .store_context
                        .memoizer(|cache: &mut TextLogIndexCache| {
                            cache.refine(key, cached.layout, span_id, Arc::new(exact_index))
                        })
            {
                index = updated.index;
                ctx.egui_ctx().request_repaint();
            }
        }

        // Fill visible rows (plus overscan) that have exact identities; the rest stay
        // placeholders until their span is resolved.
        let display_rows = plan.display_rows(&index);
        let mut entries = Self::populate_entries(
            ctx,
            sources,
            &index,
            display_rows,
            output,
            &mut discovered_levels,
        );
        // Level-filter choices accumulate across frames, since each frame only sees part of the data.
        let level_key = TextLogQueryKey::new(view_query.timeline, None, sources);
        entries.levels = ctx
            .viewer_ctx
            .store_context
            .memoizer(|cache: &mut TextLogLevelCache| {
                cache.extend_discovered_levels(level_key, discovered_levels)
            });
        entries.navigation = plan.finish(index);
        Ok(entries)
    }

    fn populate_entries(
        ctx: &ViewContext<'_>,
        sources: &[TextLogSource<'_>],
        index: &TextLogSpanIndex,
        display_rows: Range<usize>,
        output: &VisualizerExecutionOutput,
        discovered: &mut BTreeSet<String>,
    ) -> TextLogEntries {
        let first = display_rows.start.min(index.len());
        let end = display_rows.end.min(index.len());
        let mut entries = TextLogEntries {
            first_entry: first,
            entries: vec![None; end.saturating_sub(first)],
            ..Default::default()
        };
        for (offset, rows) in index.resolved_rows(first..end) {
            for (source_id, source) in sources.iter().enumerate() {
                source.populate(
                    ctx,
                    source_id,
                    rows,
                    &mut entries.entries[offset - first..offset - first + rows.len()],
                    output,
                    discovered,
                );
            }
        }
        entries
    }
}
