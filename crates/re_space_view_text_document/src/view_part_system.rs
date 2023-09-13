use re_arrow_store::LatestAtQuery;
use re_query::{query_archetype, QueryError};
use re_types::Archetype as _;
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

// ---

#[derive(Debug, Clone)]
pub struct TextDocumentEntry {
    pub body: re_types::datatypes::Utf8,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct TextDocumentSystem {
    pub text_entries: Vec<TextDocumentEntry>,
}

impl NamedViewSystem for TextDocumentSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "TextDocument".into()
    }
}

impl ViewPartSystem for TextDocumentSystem {
    fn archetype(&self) -> ArchetypeDefinition {
        re_types::archetypes::TextDocument::all_components()
            .try_into()
            .unwrap()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = &ctx.store_db.entity_db.data_store;

        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for (ent_path, _props) in query.iter_entities_for_system(Self::name()) {
            // TOOD(jleibs): this match can go away once we resolve:
            // https://github.com/rerun-io/rerun/issues/3320
            match query_archetype::<re_types::archetypes::TextDocument>(
                store,
                &timeline_query,
                ent_path,
            ) {
                Ok(arch_view) => {
                    for text_entry in
                        arch_view.iter_required_component::<re_types::components::Text>()?
                    {
                        let re_types::components::Text(text) = text_entry;
                        self.text_entries.push(TextDocumentEntry { body: text });
                    }
                }
                Err(QueryError::PrimaryNotFound(_)) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            };
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
