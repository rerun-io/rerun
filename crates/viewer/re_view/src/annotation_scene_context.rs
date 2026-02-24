use std::sync::Arc;

use re_viewer_context::{
    AnnotationMap, IdentifiedViewSystem, ViewContextSystem, ViewContextSystemOncePerFrameResult,
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
    fn execute_once_per_frame(
        ctx: &re_viewer_context::ViewerContext<'_>,
    ) -> ViewContextSystemOncePerFrameResult {
        // Use static execution to load the annotation map for all entities.
        // Alternatively, we could do this only for visible ones per View but this is actually a lot more expensive to do
        // given that there's typically just one annotation map per recording anyways!
        let mut annotation_map = AnnotationMap::default();
        annotation_map.load(ctx.recording(), &ctx.current_query());

        Box::new(Self(Arc::new(annotation_map)))
    }

    fn execute(
        &mut self,
        _ctx: &re_viewer_context::ViewContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        _query: &re_viewer_context::ViewQuery<'_>,
        once_per_frame_result: &ViewContextSystemOncePerFrameResult,
    ) {
        // Take over the static result to make it available.
        self.0 = once_per_frame_result
            .downcast_ref::<Self>()
            .expect("Unexpected static execution result type")
            .0
            .clone();
    }
}
