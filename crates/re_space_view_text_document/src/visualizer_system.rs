use re_data_store::LatestAtQuery;
use re_types::{archetypes::TextDocument, components};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContext, ViewContextCollection,
    ViewQuery, VisualizerQueryInfo, VisualizerSystem,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let Some(text) = ctx
                .recording()
                .latest_at_component::<components::Text>(&data_result.entity_path, &timeline_query)
                .map(|res| res.value)
            else {
                // Text component is required.
                continue;
            };
            let media_type = ctx
                .recording()
                .latest_at_component::<components::MediaType>(
                    &data_result.entity_path,
                    &timeline_query,
                )
                .map(|res| res.value);

            let media_type = media_type.unwrap_or(components::MediaType::plain_text());
            self.text_entries.push(TextDocumentEntry {
                body: text.clone(),
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
