use itertools::izip;
use re_chunk_store::AbsoluteTimeRange;
use re_entity_db::EntityPath;
use re_log_types::{TimeInt, TimePoint};
use re_query::{clamped_zip_1x2, range_zip_1x2};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_view::{RangeResultsExt as _, range_with_blueprint_resolved_data};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub entity_path: EntityPath,
    pub time: TimeInt,
    pub timepoint: TimePoint,
    pub color: Option<Color>,
    pub body: Text,
    pub level: Option<TextLogLevel>,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextLogSystem {
    pub entries: Vec<Entry>,
}

impl IdentifiedViewSystem for TextLogSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TextLog".into()
    }
}

impl VisualizerSystem for TextLogSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<TextLog>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let query =
            re_chunk_store::RangeQuery::new(view_query.timeline, AbsoluteTimeRange::EVERYTHING)
                .keep_extra_timelines(true);

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            self.process_visualizer_instruction(ctx, &query, data_result, instruction);
        }

        {
            // Sort by currently selected timeline
            re_tracing::profile_scope!("sort");
            self.entries.sort_by_key(|e| e.time);
        }

        Ok(VisualizerExecutionOutput::default())
    }
}

impl TextLogSystem {
    fn process_visualizer_instruction(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &re_chunk_store::RangeQuery,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
    ) {
        re_tracing::profile_function!();

        let results = range_with_blueprint_resolved_data(
            ctx,
            None,
            query,
            data_result,
            TextLog::all_component_identifiers(),
            instruction,
        );

        let Some(all_text_chunks) =
            results.get_required_chunks(TextLog::descriptor_text().component)
        else {
            return;
        };

        // TODO(cmc): It would be more efficient (both space and compute) to do this lazily as
        // we're rendering the table by indexing back into the original chunk etc.
        // Let's keep it simple for now, until we have data suggested we need the extra perf.
        let all_timepoints = all_text_chunks
            .iter()
            .flat_map(|chunk| chunk.iter_component_timepoints());

        let timeline = *query.timeline();
        let all_texts = results.iter_as(timeline, TextLog::descriptor_text().component);
        let all_levels = results.iter_as(timeline, TextLog::descriptor_level().component);
        let all_colors = results.iter_as(timeline, TextLog::descriptor_color().component);

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
                self.entries.push(Entry {
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
