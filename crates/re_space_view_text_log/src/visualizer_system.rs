use re_data_store::ResolvedTimeRange;
use re_entity_db::EntityPath;
use re_log_types::{RowId, TimeInt};
use re_query::{clamped_zip_1x2, range_zip_1x2};
use re_space_view::{range_with_blueprint_resolved_data, RangeResultsExt};
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    Loggable as _,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub row_id: RowId,

    pub entity_path: EntityPath,

    pub time: TimeInt,

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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let query =
            re_data_store::RangeQuery::new(view_query.timeline, ResolvedTimeRange::EVERYTHING);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            if let Err(err) = self.process_entity(ctx, &query, data_result) {
                re_log::error_once!(
                    "Error visualizing text logs for {:?}: {:?}",
                    data_result.entity_path,
                    err
                );
            }
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

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TextLogSystem {
    fn process_entity(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &re_data_store::RangeQuery,
        data_result: &re_viewer_context::DataResult,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();
        let resolver = ctx.recording().resolver();

        let results = range_with_blueprint_resolved_data(
            ctx,
            None,
            query,
            data_result,
            [Text::name(), TextLogLevel::name(), Color::name()],
        );

        let Some(all_texts) = results.get_dense::<Text>(resolver).transpose()? else {
            return Ok(());
        };

        let all_levels = results.get_or_empty_dense::<TextLogLevel>(resolver)?;
        let all_colors = results.get_or_empty_dense::<Color>(resolver)?;
        let all_frames = range_zip_1x2(
            all_texts.range_indexed(),
            all_levels.range_indexed(),
            all_colors.range_indexed(),
        );

        for (&(data_time, row_id), bodies, levels, colors) in all_frames {
            let levels = levels.unwrap_or(&[]).iter().cloned().map(Some);
            let colors = colors.unwrap_or(&[]).iter().copied().map(Some);

            let level_default_fn = || None;
            let color_default_fn = || None;

            let results =
                clamped_zip_1x2(bodies, levels, level_default_fn, colors, color_default_fn);

            for (text, level, color) in results {
                self.entries.push(Entry {
                    row_id,
                    entity_path: data_result.entity_path.clone(),
                    time: data_time,
                    color,
                    body: text.clone(),
                    level,
                });
            }
        }

        Ok(())
    }
}

re_viewer_context::impl_component_fallback_provider!(TextLogSystem => []);
