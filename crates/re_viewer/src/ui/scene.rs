use nohash_hasher::IntSet;

use re_data_store::{ObjPath, ObjectProps, ObjectsProperties, TimeInt, Timeline};

// ---

pub struct SceneQuery<'s> {
    pub obj_paths: &'s IntSet<ObjPath>,
    pub timeline: Timeline,
    pub latest_at: TimeInt,
    pub obj_props: &'s ObjectsProperties,
}

impl<'s> SceneQuery<'s> {
    /// Iter over all of the currently visible `EntityPath`s in the `SceneQuery`
    ///
    /// Also includes the corresponding `ObjectProps`.
    pub(crate) fn iter_entities(&self) -> impl Iterator<Item = (&ObjPath, ObjectProps)> {
        self.obj_paths
            .iter()
            .map(|obj_path| (obj_path, self.obj_props.get(obj_path)))
            .filter(|(_obj_path, obj_props)| obj_props.visible)
    }
}
