use crate::{ScenePartCollection, SpaceViewClass};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
pub struct TypedScene<C: SpaceViewClass> {
    pub context: <<C as SpaceViewClass>::SceneParts as ScenePartCollection>::Context,
    pub parts: C::SceneParts,

    /// All draw data gathered during the last call to [`Self::populate`].
    ///
    /// TODO(wumpf): Right now the ui methods control when and how to create [`re_renderer::ViewBuilder`]s.
    ///              In the future, we likely want to move view builder handling to `re_viewport` with
    ///              minimal configuration options exposed via [`crate::SpaceViewClass`].
    pub draw_data: Vec<re_renderer::QueueableDrawData>,
}

impl<C: SpaceViewClass> Default for TypedScene<C> {
    fn default() -> Self {
        Self {
            context: Default::default(),
            parts: Default::default(),
            draw_data: Default::default(),
        }
    }
}
