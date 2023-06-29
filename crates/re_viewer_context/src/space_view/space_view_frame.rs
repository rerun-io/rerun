use crate::{ScenePartCollection, SpaceViewClass};

/// Every [`crate::SpaceViewClass`] creates and populates a scene to draw a frame and inform the ui about relevant data.
///
/// When populating a scene, first all contexts are populated,
/// and then all elements with read access to the previously established context objects.
pub struct SpaceViewFrame<C: SpaceViewClass> {
    pub context: <<C as SpaceViewClass>::SceneParts as ScenePartCollection>::Context,
    pub parts: C::SceneParts,
}

impl<C: SpaceViewClass> Default for SpaceViewFrame<C> {
    fn default() -> Self {
        Self {
            context: Default::default(),
            parts: Default::default(),
        }
    }
}
