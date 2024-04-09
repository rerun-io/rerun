use re_data_store::{RangeQuery, TimeRange};
use re_entity_db::EntityPath;
use re_log_types::{RowId, TimeInt};
use re_query_cache2::{clamped_zip_1x2, range_zip_1x2, CachedRangeData, PromiseResult};
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    Component, Loggable as _,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

#[derive(Debug, Clone)]
pub struct Entry {
    // props
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
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = ctx.recording_store();
        let query_caches2 = ctx.recording().query_caches2();
        let resolver = ctx.recording().resolver();

        // We want everything, for all times:
        let query = re_data_store::RangeQuery::new(view_query.timeline, TimeRange::EVERYTHING);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let results = query_caches2.range(
                store,
                &query,
                &data_result.entity_path,
                [Text::name(), TextLogLevel::name(), Color::name()],
            );

            let all_bodies = {
                let Some(all_bodies) = results.get(Text::name()) else {
                    continue;
                };
                all_bodies.to_dense::<Text>(resolver)
            };
            check_range(&query, &all_bodies)?;

            let all_levels = results
                .get_or_empty(TextLogLevel::name())
                .to_dense::<TextLogLevel>(resolver);
            check_range(&query, &all_levels)?;

            let all_colors = results
                .get_or_empty(Color::name())
                .to_dense::<Color>(resolver);
            check_range(&query, &all_colors)?;

            let all_frames = range_zip_1x2(
                all_bodies.range_indexed(query.range()),
                all_levels.range_indexed(query.range()),
                all_colors.range_indexed(query.range()),
            );

            for (&(data_time, row_id), bodies, levels, colors) in all_frames {
                let levels = levels.unwrap_or(&[]).iter().cloned().map(Some);
                let colors = colors.unwrap_or(&[]).iter().copied().map(Some);

                let level_default_fn = || None;
                let color_default_fn = || None;

                let results =
                    clamped_zip_1x2(bodies, levels, level_default_fn, colors, color_default_fn);

                for (body, level, color) in results {
                    self.entries.push(Entry {
                        row_id,
                        entity_path: data_result.entity_path.clone(),
                        time: data_time,
                        color,
                        body: body.clone(),
                        level,
                    });
                }
            }
        }

        {
            // Sort by currently selected tiemeline
            re_tracing::profile_scope!("sort");
            self.entries.sort_by_key(|e| e.time);
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// TODO(#5607): what should happen if the promise is still pending?
#[inline]
fn check_range<'a, C: Component>(
    query: &RangeQuery,
    results: &'a CachedRangeData<'a, C>,
) -> re_query_cache2::Result<()> {
    let (front_status, back_status) = results.status(query.range());
    match front_status {
        PromiseResult::Pending => return Ok(()),
        PromiseResult::Error(err) => return Err(re_query_cache2::QueryError::Other(err.into())),
        PromiseResult::Ready(_) => {}
    }
    match back_status {
        PromiseResult::Pending => return Ok(()),
        PromiseResult::Error(err) => return Err(re_query_cache2::QueryError::Other(err.into())),
        PromiseResult::Ready(_) => {}
    }

    Ok(())
}
