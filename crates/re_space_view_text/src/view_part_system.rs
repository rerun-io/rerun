use re_arrow_store::TimeRange;
use re_data_store::EntityPath;
use re_log_types::RowId;
use re_query::{range_entity_with_primary, QueryError};
use re_types::{components::InstanceKey, Loggable as _};
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

#[derive(Debug, Clone)]
pub struct TextEntry {
    // props
    pub row_id: RowId,

    pub entity_path: EntityPath,

    /// `None` for timeless data.
    pub time: Option<i64>,

    pub color: Option<[u8; 4]>,

    pub level: Option<String>,

    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextSystem {
    pub text_entries: Vec<TextEntry>,
}

impl NamedViewSystem for TextSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "Text".into()
    }
}

impl ViewPartSystem for TextSystem {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![re_components::TextEntry::name()]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, _) in query.iter_entities_for_system(Self::name()) {
            let query = re_arrow_store::RangeQuery::new(
                query.timeline,
                TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            );

            let components = [
                InstanceKey::name(),
                re_components::TextEntry::name(),
                re_types::components::Color::name(),
            ];
            let ent_views = range_entity_with_primary::<re_components::TextEntry, 3>(
                store, &query, ent_path, components,
            );

            for (time, ent_view) in ent_views {
                match ent_view.visit2(
                    |_instance,
                     text_entry: re_components::TextEntry,
                     color: Option<re_types::components::Color>| {
                        let re_components::TextEntry { body, level } = text_entry;

                        self.text_entries.push(TextEntry {
                            row_id: ent_view.primary_row_id(),
                            entity_path: ent_path.clone(),
                            time: time.map(|time| time.as_i64()),
                            color: color.map(|c| c.to_array()),
                            level,
                            body,
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
            self.text_entries.sort_by_key(|entry| entry.time);
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
