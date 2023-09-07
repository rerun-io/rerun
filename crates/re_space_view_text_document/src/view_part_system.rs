use re_arrow_store::LatestAtQuery;
use re_query::query_archetype;
use re_types::{Archetype as _, Loggable as _};
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
        // TODO(#3159): use actual archetype definition
        // TextDocument::all_components().try_into().unwrap()
        vec1::vec1![
            re_types::archetypes::TextDocument::indicator_component(),
            re_types::components::Text::name(),
        ]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = &ctx.store_db.entity_db.data_store;

        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for (ent_path, props) in query.iter_entities_for_system(Self::name()) {
            if !props.visible {
                continue;
            }

            let arch_view = query_archetype::<re_types::archetypes::TextDocument>(
                store,
                &timeline_query,
                ent_path,
            )?;

            // TODO(emilk): use `iter_required_component` instead, once it doesn't require Default
            for text_entry in arch_view
                .iter_optional_component::<re_types::components::Text>()?
                .flatten()
            {
                let re_types::components::Text(text) = text_entry;
                self.text_entries.push(TextDocumentEntry { body: text });
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
