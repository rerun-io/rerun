use re_components::AnnotationContext;
use re_log_types::Component;
use re_viewer_context::{AnnotationMap, ArchetypeDefinition, SceneContext};

#[derive(Default)]
pub struct AnnotationSceneContext(pub AnnotationMap);

impl SceneContext for AnnotationSceneContext {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
