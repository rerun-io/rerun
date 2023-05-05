use nohash_hasher::IntSet;

use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap, TimeInt, Timeline};

pub struct SceneQuery<'s> {
    pub entity_paths: &'s IntSet<EntityPath>,
    pub timeline: Timeline,
    pub latest_at: TimeInt,
    pub entity_props_map: &'s EntityPropertyMap,
}

impl<'s> SceneQuery<'s> {
    /// Iter over all of the currently visible [`EntityPath`]s in the [`SceneQuery`].
    ///
    /// Also includes the corresponding [`EntityProperties`].
    pub fn iter_entities(&self) -> impl Iterator<Item = (&EntityPath, EntityProperties)> {
        self.entity_paths
            .iter()
            .map(|entity_path| (entity_path, self.entity_props_map.get(entity_path)))
            .filter(|(_entity_path, props)| props.visible)
    }
}
