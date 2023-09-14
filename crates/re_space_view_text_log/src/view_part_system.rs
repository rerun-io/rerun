use re_arrow_store::TimeRange;
use re_data_store::EntityPath;
use re_log_types::RowId;
use re_query::{range_entity_with_primary, QueryError};
use re_types::{
    archetypes::TextLog,
    components::{Color, InstanceKey, Text, TextLogLevel},
    Archetype as _, Loggable as _,
};
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
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
    fn archetype(&self) -> ArchetypeDefinition {
        TextLog::all_components().try_into().unwrap()
    }

    fn queries_any_components_of(
        &self,
        _store: &re_arrow_store::DataStore,
        _ent_path: &EntityPath,
        components: &[re_types::ComponentName],
    ) -> bool {
        components.contains(&TextLog::indicator_component())
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, _) in query.iter_entities_for_system(Self::name()) {
            // We want everything, for all times:
            let timeline_query =
                re_arrow_store::RangeQuery::new(query.timeline, TimeRange::EVERYTHING);

            let components = [
                InstanceKey::name(),
                Text::name(),
                TextLogLevel::name(),
                Color::name(),
            ];
            let ent_views =
                range_entity_with_primary::<Text, 4>(store, &timeline_query, ent_path, components);

            for (time, ent_view) in ent_views {
                match ent_view.visit3(
                    |_instance_key,
                     body: Text,
                     level: Option<TextLogLevel>,
                     color: Option<Color>| {
                        self.entries.push(Entry {
                            row_id: ent_view.primary_row_id(),
                            entity_path: ent_path.clone(),
                            time: time.map(|time| time.as_i64()),
                            color,
                            body,
                            level,
                        });
                    },
                ) {
                    Ok(_) | Err(QueryError::PrimaryNotFound(_)) => {}
                    Err(err) => {
                        re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                    }
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
