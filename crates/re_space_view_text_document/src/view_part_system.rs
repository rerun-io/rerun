use re_arrow_store::LatestAtQuery;
use re_query::{query_archetype, QueryError};
use re_types::{
    archetypes::{self, TextDocument},
    components, Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
};

// ---

#[derive(Debug, Clone)]
pub struct TextDocumentEntry {
    pub body: components::Text,
    pub media_type: components::MediaType,
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
    fn required_components(&self) -> ComponentNameSet {
        TextDocument::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(TextDocument::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let store = ctx.store_db.store();

        let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

        for data_result in query.iter_visible_data_results(Self::name()) {
            // TODO(jleibs): this match can go away once we resolve:
            // https://github.com/rerun-io/rerun/issues/3320
            match query_archetype::<archetypes::TextDocument>(
                store,
                &timeline_query,
                &data_result.entity_path,
            ) {
                Ok(arch_view) => {
                    let bodies = arch_view.iter_required_component::<components::Text>()?;
                    let media_types =
                        arch_view.iter_optional_component::<components::MediaType>()?;

                    for (body, media_type) in itertools::izip!(bodies, media_types) {
                        let media_type = media_type.unwrap_or(components::MediaType::plain_text());
                        self.text_entries
                            .push(TextDocumentEntry { body, media_type });
                    }
                }
                Err(QueryError::PrimaryNotFound(_)) => {}
                Err(err) => {
                    re_log::error_once!(
                        "Unexpected error querying {:?}: {err}",
                        &data_result.entity_path
                    );
                }
            };
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
