use re_data_store::TimeRange;
use re_entity_db::EntityPath;
use re_log_types::RowId;
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
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

    /// `None` for timeless data.
    pub time: Option<i64>,

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
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let query_caches = ctx.entity_db.query_caches();
        let store = ctx.entity_db.store();

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            // We want everything, for all times:
            let timeline_query =
                re_data_store::RangeQuery::new(query.timeline, TimeRange::EVERYTHING);

            // TODO(cmc): use raw API.
            query_caches.query_archetype_pov1_comp2::<TextLog, Text, TextLogLevel, Color, _>(
                store,
                &timeline_query.clone().into(),
                &data_result.entity_path,
                |((time, row_id), _, bodies, levels, colors)| {
                    for (body, level, color) in itertools::izip!(
                        bodies.iter(),
                        re_query_cache::iter_or_repeat_opt(levels, bodies.len()),
                        re_query_cache::iter_or_repeat_opt(colors, bodies.len()),
                    ) {
                        self.entries.push(Entry {
                            row_id,
                            entity_path: data_result.entity_path.clone(),
                            time: time.map(|time| time.as_i64()),
                            color: *color,
                            body: body.clone(),
                            level: level.clone(),
                        });
                    }
                },
            )?;
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
