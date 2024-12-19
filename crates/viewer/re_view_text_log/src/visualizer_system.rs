use itertools::izip;
use re_chunk_store::ResolvedTimeRange;
use re_entity_db::EntityPath;
use re_log_types::TimeInt;
use re_log_types::TimePoint;
use re_query::{clamped_zip_1x2, range_zip_1x2};
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    Component as _,
};
use re_view::{range_with_blueprint_resolved_data, RangeResultsExt};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerQueryInfo, VisualizerSystem,
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
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<TextLog>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let query =
            re_chunk_store::RangeQuery::new(view_query.timeline, ResolvedTimeRange::EVERYTHING)
                .keep_extra_timelines(true);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            self.process_entity(ctx, &query, data_result);
        }

        {
            // Sort by currently selected timeline
            re_tracing::profile_scope!("sort");
            self.entries.sort_by_key(|e| e.time);
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TextLogSystem {
    fn process_entity(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &re_chunk_store::RangeQuery,
        data_result: &re_viewer_context::DataResult,
    ) {
        re_tracing::profile_function!();

        let results = range_with_blueprint_resolved_data(
            ctx,
            None,
            query,
            data_result,
            [Text::name(), TextLogLevel::name(), Color::name()],
        );

        let Some(all_text_chunks) = results.get_required_chunks(&Text::name()) else {
            return;
        };

        // TODO(cmc): It would be more efficient (both space and compute) to do this lazily as
        // we're rendering the table by indexing back into the original chunk etc.
        // Let's keep it simple for now, until we have data suggested we need the extra perf.
        let all_timepoints = all_text_chunks
            .iter()
            .flat_map(|chunk| chunk.iter_component_timepoints(&Text::name()));

        let timeline = query.timeline();
        let all_texts = results.iter_as(timeline, Text::name());
        let all_levels = results.iter_as(timeline, TextLogLevel::name());
        let all_colors = results.iter_as(timeline, Color::name());

        let all_frames = range_zip_1x2(
            all_texts.string(),
            all_levels.string(),
            all_colors.primitive::<u32>(),
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

re_viewer_context::impl_component_fallback_provider!(TextLogSystem => []);
