use re_arrow_store::TimeRange;
use re_data_store::EntityPath;
use re_log_types::RowId;
use re_query::range_archetype;
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
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

impl NamedViewSystem for TextLogSystem {
    fn name() -> re_viewer_context::ViewSystemName {
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
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = ctx.store_db.store();

        for data_result in query.iter_visible_data_results(Self::name()) {
            // We want everything, for all times:
            let timeline_query =
                re_arrow_store::RangeQuery::new(query.timeline, TimeRange::EVERYTHING);

            let arch_views = range_archetype::<TextLog, { TextLog::NUM_COMPONENTS }>(
                store,
                &timeline_query,
                &data_result.entity_path,
            );

            for (time, arch_view) in arch_views {
                let bodies = arch_view.iter_required_component::<Text>()?;
                let levels = arch_view.iter_optional_component::<TextLogLevel>()?;
                let colors = arch_view.iter_optional_component::<Color>()?;

                for (body, level, color) in itertools::izip!(bodies, levels, colors) {
                    self.entries.push(Entry {
                        row_id: arch_view.primary_row_id(),
                        entity_path: data_result.entity_path.clone(),
                        time: time.map(|time| time.as_i64()),
                        color,
                        body,
                        level,
                    });
                }
            }
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
