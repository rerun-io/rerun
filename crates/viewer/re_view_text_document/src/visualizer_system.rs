use re_chunk_store::LatestAtQuery;
use re_types::{
    archetypes::TextDocument,
    components::{self},
};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, TypedComponentFallbackProvider, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerQueryInfo, VisualizerSystem,
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
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<TextDocument>(ctx, &timeline_query);

            let Some(text) =
                results.get_required_mono::<components::Text>(&TextDocument::descriptor_text())
            else {
                continue;
            };
            self.text_entries.push(TextDocumentEntry {
                body: text.clone(),
                media_type: results
                    .get_mono_with_fallback(&TextDocument::descriptor_media_type(), self),
            });
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<components::MediaType> for TextDocumentSystem {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> components::MediaType {
        components::MediaType::plain_text()
    }
}

re_viewer_context::impl_component_fallback_provider!(TextDocumentSystem => [components::MediaType]);
