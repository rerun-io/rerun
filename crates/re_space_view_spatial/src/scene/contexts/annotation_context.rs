use re_components::DrawOrder;
use re_log_types::{Component, ComponentName};
use re_viewer_context::{AnnotationMap, SceneContext};

#[derive(Default)]
pub struct AnnotationSceneContext(pub AnnotationMap);

impl SceneContext for AnnotationSceneContext {
    fn component_names(&self) -> Vec<ComponentName> {
        vec![DrawOrder::name()]
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
