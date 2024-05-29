use re_data_store::LatestAtQuery;
use re_space_view::external::re_query::PromiseResult;
use re_types::{archetypes::TextDocument, components};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
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

impl IdentifiedViewSystem for TextDocumentSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TextDocument".into()
    }
}

impl VisualizerSystem for TextDocumentSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<TextDocument>()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let TextDocument { text, media_type } = match ctx
                .recording()
                .latest_at_archetype(&data_result.entity_path, &timeline_query)
            {
                PromiseResult::Pending | PromiseResult::Ready(None) => {
                    // TODO(#5607): what should happen if the promise is still pending?
                    continue;
                }
                PromiseResult::Ready(Some((_, arch))) => arch,
                PromiseResult::Error(err) => {
                    re_log::error_once!(
                        "Unexpected error querying {:?}: {err}",
                        &data_result.entity_path
                    );
                    continue;
                }
            };

            let media_type = media_type.unwrap_or(components::MediaType::plain_text());
            self.text_entries.push(TextDocumentEntry {
                body: text,
                media_type,
            });
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(TextDocumentSystem => []);
