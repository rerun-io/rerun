use re_arrow_store::TimeRange;
use re_data_store::EntityPath;
use re_log_types::RowId;
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
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

impl ViewPartSystem for TextLogSystem {
    fn required_components(&self) -> ComponentNameSet {
        TextLog::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(TextLog::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = ctx.store_db.store();

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            // We want everything, for all times:
            let timeline_query =
                re_arrow_store::RangeQuery::new(query.timeline, TimeRange::EVERYTHING);

            re_query_cache::query_cached_archetype_r1o2::<
                { TextLog::NUM_COMPONENTS },
                TextLog,
                Text,
                TextLogLevel,
                Color,
                _,
            >(
                store,
                &timeline_query.clone().into(),
                &data_result.entity_path,
                |it| {
                    for ((time, row_id), _, bodies, levels, colors) in it {
                        for (body, level, color) in itertools::izip!(bodies, levels, colors) {
                            self.entries.push(Entry {
                                row_id: *row_id,
                                entity_path: data_result.entity_path.clone(),
                                time: Some(time.as_i64()), // TODO: timeless
                                color: *color,
                                body: body.clone(),
                                level: level.clone(),
                            });
                        }
                    }
                },
            );
        }

        {
            re_tracing::profile_scope!("sort");
            self.entries.sort_by_key(|entry| entry.time);
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
