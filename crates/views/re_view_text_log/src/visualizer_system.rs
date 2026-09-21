use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use itertools::izip;
use re_chunk_store::AbsoluteTimeRange;
use re_entity_db::EntityPath;
use re_log_types::hash::Hash64;
use re_log_types::{TimeInt, TimePoint};
use re_query::{clamped_zip_1x2, range_zip_1x2};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_view::{VisualizerInstructionQueryResults, range_with_blueprint_resolved_data};
use re_viewer_context::{
    Cache, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};
use re_viewport_blueprint::ViewProperty;

#[derive(Debug, Clone, re_byte_size::SizeBytes)]
pub struct Entry {
    pub entity_path: EntityPath,
    pub time: TimeInt,
    pub timepoint: TimePoint,
    pub color: Option<Color>,
    pub body: Text,
    pub level: Option<TextLogLevel>,
}

/// Output of [`TextLogSystem`].
#[derive(Default, re_byte_size::SizeBytes)]
pub struct TextLogEntries {
    /// Rows passing the level filter, sorted by time on the query timeline.
    pub entries: Vec<Entry>,

    /// Every level in the data, including the filtered-out ones.
    pub levels: BTreeSet<String>,
}

struct TextLogEntryCacheEntry {
    entries: Arc<TextLogEntries>,
    last_used_generation: u64,
}

/// Memoizes [`TextLogEntries`] keyed on the query results and level filter that produced them.
///
/// Entries not used in the previous frame are dropped, since the key covers blueprint
/// overrides and there is no other way to tell which entries became unreachable.
#[derive(Default)]
pub struct TextLogEntryCache {
    cache: HashMap<Hash64, TextLogEntryCacheEntry>,
    generation: u64,
}

impl TextLogEntryCache {
    fn entry(
        &mut self,
        key: Hash64,
        compute: impl FnOnce() -> TextLogEntries,
    ) -> Arc<TextLogEntries> {
        let entry = self
            .cache
            .entry(key)
            .or_insert_with(|| TextLogEntryCacheEntry {
                entries: Arc::new(compute()),
                last_used_generation: 0,
            });
        entry.last_used_generation = self.generation;
        entry.entries.clone()
    }
}

impl Cache for TextLogEntryCache {
    fn name(&self) -> &'static str {
        "TextLogEntryCache"
    }

    fn begin_frame(&mut self) {
        self.cache
            .retain(|_, entry| entry.last_used_generation == self.generation);
        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        self.cache.clear();
    }
}

impl re_byte_size::MemUsageTreeCapture for TextLogEntryCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        use re_byte_size::SizeBytes as _;
        let bytes = self
            .cache
            .values()
            .map(|entry| entry.entries.total_size_bytes())
            .sum();
        re_byte_size::MemUsageTree::Bytes(bytes)
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

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let query =
            re_chunk_store::RangeQuery::new(view_query.timeline, AbsoluteTimeRange::EVERYTHING)
                .keep_extra_timelines(true);

        // No blueprint value means no filter; the fallback would list every level anyway.
        let filter = ViewProperty::from_archetype::<TextLogRows>(ctx)
            .component_array::<TextLogLevel>(
                TextLogRows::descriptor_filter_by_log_level().component,
            )?
            .map(|levels| {
                levels
                    .iter()
                    .map(|lvl| lvl.as_str().to_owned())
                    .collect::<BTreeSet<_>>()
            });

        // The query runs every frame so that missing chunks keep being reported;
        // only the conversion to entries is memoized.
        let mut per_instruction = Vec::new();
        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let range_results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                TextLog::all_component_identifiers(),
                instruction,
            );
            let results = re_view::BlueprintResolvedResults::from((query.clone(), range_results));
            per_instruction.push((data_result, instruction, results));
        }

        let mut sources = Vec::with_capacity(per_instruction.len());
        let mut hashes = Vec::with_capacity(per_instruction.len());
        for (data_result, instruction, results) in &per_instruction {
            let results = VisualizerInstructionQueryResults::new(instruction, results, &output);
            if results
                .iter_required(TextLog::descriptor_text().component)
                .is_empty()
            {
                continue;
            }
            hashes.push(results.query_result_hash());
            sources.push((*data_result, results));
        }
        let key = Hash64::hash((view_query.timeline, &filter, &hashes));

        let entries = ctx
            .viewer_ctx
            .store_context
            .memoizer(|cache: &mut TextLogEntryCache| {
                cache.entry(key, || {
                    re_tracing::profile_scope!("build entries");

                    let mut entries = TextLogEntries::default();
                    for (data_result, results) in &sources {
                        Self::process_visualizer_instruction(
                            &mut entries,
                            filter.as_ref(),
                            data_result,
                            results,
                        );
                    }

                    {
                        // Sort by currently selected timeline
                        re_tracing::profile_scope!("sort");
                        entries.entries.sort_by_key(|e| e.time);
                    }
                    entries
                })
            });

        Ok(output.with_visualizer_data(entries))
    }
}

impl TextLogSystem {
    fn process_visualizer_instruction(
        entries: &mut TextLogEntries,
        filter: Option<&BTreeSet<String>>,
        data_result: &re_viewer_context::DataResult,
        results: &VisualizerInstructionQueryResults<'_>,
    ) {
        re_tracing::profile_function!();

        let all_texts = results.iter_required(TextLog::descriptor_text().component);

        // TODO(cmc): It would be more efficient (both space and compute) to do this lazily as
        // we're rendering the table by indexing back into the original chunk etc.
        // Let's keep it simple for now, until we have data suggested we need the extra perf.
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
            let levels = levels.as_deref().unwrap_or(&[]).iter().cloned().map(Some);
            let colors = colors
                .unwrap_or(&[])
                .iter()
                .copied()
                .map(Into::into)
                .map(Some);

            let level_default_fn = || None;
            let color_default_fn = || None;

            let results =
                clamped_zip_1x2(bodies, levels, level_default_fn, colors, color_default_fn);

            for (text, level, color) in results {
                if let Some(level) = &level {
                    let level = level.as_str();
                    if !entries.levels.contains(level) {
                        entries.levels.insert(level.to_owned());
                    }
                    if filter.is_some_and(|filter| !filter.contains(level)) {
                        continue;
                    }
                }

                entries.entries.push(Entry {
                    entity_path: data_result.entity_path.clone(),
                    time: data_time,
                    timepoint: timepoint.clone(),
                    color,
                    body: text.clone().into(),
                    level: level.clone().map(Into::into),
                });
            }
        }
    }
}
