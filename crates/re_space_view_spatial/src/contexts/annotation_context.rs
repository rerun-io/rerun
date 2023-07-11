use re_components::AnnotationContext;
use re_log_types::Component;
use re_viewer_context::{AnnotationMap, ArchetypeDefinition, ViewContextSystem};

#[derive(Default)]
pub struct AnnotationSceneContext(pub AnnotationMap);

impl ViewContextSystem for AnnotationSceneContext {
    fn archetypes(&self) -> Vec<ArchetypeDefinition> {
        vec![vec1::vec1![AnnotationContext::name()]]
    }

    fn populate(
        &mut self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::SceneQuery<'_>,
        _space_view_state: &dyn re_viewer_context::SpaceViewState,
    ) {
        self.0.load(ctx, query);
    }
}
