use re_arrow_store::LatestAtQuery;
use re_query::{query_entity_with_primary, QueryError};
use re_types::Loggable as _;
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

// ---

#[derive(Debug, Clone)]
pub struct TextBoxEntry {
    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextBoxSystem {
    pub text_entries: Vec<TextBoxEntry>,
}

impl NamedViewSystem for TextBoxSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "TextBox".into()
    }
}

impl ViewPartSystem for TextBoxSystem {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![re_components::TextBox::name()]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities_for_system(Self::name()) {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            match query_entity_with_primary::<re_components::TextBox>(store, &query, ent_path, &[])
                .and_then(|ent_view| {
                    for text_entry in ent_view.iter_primary()?.flatten() {
                        let re_components::TextBox { body } = text_entry;
                        self.text_entries.push(TextBoxEntry { body });
                    }
                    Ok(())
                }) {
                Ok(_) | Err(QueryError::PrimaryNotFound(_)) => {}
                Err(_) => {
                    re_log::warn_once!("text-box query failed for {ent_path:?}");
                }
            }
        }
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
