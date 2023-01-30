use nohash_hasher::IntSet;

use re_data_store::{EntityPath, ObjectProps, ObjectsProperties, TimeInt, Timeline};

// ---

pub struct SceneQuery<'s> {
    pub entity_paths: &'s IntSet<EntityPath>,
    pub timeline: Timeline,
    pub latest_at: TimeInt,
    pub obj_props: &'s ObjectsProperties,
}

impl<'s> SceneQuery<'s> {
    /// Iter over all of the currently visible `EntityPath`s in the `SceneQuery`
    ///
    /// Also includes the corresponding `ObjectProps`.
    pub(crate) fn iter_entities(&self) -> impl Iterator<Item = (&EntityPath, ObjectProps)> {
        self.entity_paths
            .iter()
            .map(|entity_path| (entity_path, self.obj_props.get(entity_path)))
            .filter(|(_entity_path, obj_props)| obj_props.visible)
    }
}
