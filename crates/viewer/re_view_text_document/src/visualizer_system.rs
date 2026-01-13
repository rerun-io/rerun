use re_chunk_store::LatestAtQuery;
use re_sdk_types::archetypes::TextDocument;
use re_sdk_types::components;
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
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
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let results = data_result.latest_at_with_blueprint_resolved_data::<TextDocument>(
                ctx,
                &timeline_query,
                Some(instruction),
            );

            let Some(text) = results
                .get_required_mono::<components::Text>(TextDocument::descriptor_text().component)
            else {
                continue;
            };
            self.text_entries.push(TextDocumentEntry {
                body: text.clone(),
                media_type: results
                    .get_mono_with_fallback(TextDocument::descriptor_media_type().component),
            });
        }

        Ok(VisualizerExecutionOutput::default())
    }
}
