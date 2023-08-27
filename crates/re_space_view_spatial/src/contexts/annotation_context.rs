use re_types::{components::AnnotationContext, Loggable};
use re_viewer_context::{AnnotationMap, ArchetypeDefinition, ViewContextSystem};

#[derive(Default)]
pub struct AnnotationSceneContext(pub AnnotationMap);

impl ViewContextSystem for AnnotationSceneContext {
    fn archetypes(&self) -> Vec<ArchetypeDefinition> {
        vec![vec1::vec1![AnnotationContext::name()]]
    }

    fn execute(
        &mut self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        re_tracing::profile_function!();
        self.0.load(
            ctx,
            &query.latest_at_query(),
            query.iter_entities().map(|(p, _)| p),
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
