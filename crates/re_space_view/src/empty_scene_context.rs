use re_viewer_context::{SceneContext, SceneContextPart};

/// Implementation of an empty scene context.
#[derive(Default)]
pub struct EmptySceneContext;

impl SceneContext for EmptySceneContext {
    fn vec_mut(&mut self) -> Vec<&mut dyn SceneContextPart> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
