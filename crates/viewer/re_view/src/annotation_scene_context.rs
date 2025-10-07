use std::sync::Arc;

use re_viewer_context::{
    AnnotationMap, IdentifiedViewSystem, ViewContextSystem, ViewContextSystemStaticExecResult,
    ViewSystemIdentifier,
};

#[derive(Default)]
pub struct AnnotationSceneContext(pub Arc<AnnotationMap>);

impl IdentifiedViewSystem for AnnotationSceneContext {
    fn identifier() -> ViewSystemIdentifier {
        "AnnotationSceneContext".into()
    }
}

impl ViewContextSystem for AnnotationSceneContext {
    fn execute_static(
        ctx: &re_viewer_context::ViewerContext<'_>,
    ) -> ViewContextSystemStaticExecResult {
        // Use static execution to load the annotation map for all entities.
        // Alternatively, we could do this only for visible ones per View but this is actually a lot more expensive to do
        // given that there's typically just one annotation map per recording anyways!
        let mut annotation_map = AnnotationMap::default();
        annotation_map.load(ctx, &ctx.current_query());

        Box::new(Self(Arc::new(annotation_map)))
    }

    fn execute(
        &mut self,
        _ctx: &re_viewer_context::ViewContext<'_>,
        _query: &re_viewer_context::ViewQuery<'_>,
        static_execution_result: &ViewContextSystemStaticExecResult,
    ) {
        // Take over the static result to make it available.
        self.0 = static_execution_result
            .downcast_ref::<Self>()
            .expect("Unexpected static execution result type")
            .0
            .clone();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
