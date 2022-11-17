use ahash::HashMap;
use nohash_hasher::IntSet;

use re_data_store::{
    FieldName, FieldStore, LogDb, ObjPath, ObjStore, ObjectsProperties, TimeQuery, Timeline,
};
use re_log_types::ObjectType;

use super::{space_view::ViewCategory, view_2d, view_3d, view_plot, view_tensor, view_text};

// ---

/// A fully self-contained scene, ready to be rendered as-is.
#[derive(Default)]
pub struct Scene {
    pub two_d: view_2d::Scene2D,
    pub three_d: view_3d::Scene3D,
    pub text: view_text::SceneText,
    pub tensor: view_tensor::SceneTensor,
    pub plot: view_plot::ScenePlot,
}

impl Scene {
    pub(crate) fn categories(&self) -> std::collections::BTreeSet<ViewCategory> {
        let has_2d = !self.two_d.is_empty() && self.tensor.is_empty();
        let has_3d = !self.three_d.is_empty();
        let has_text = !self.text.is_empty();
        let has_tensor = !self.tensor.is_empty();

        [
            has_2d.then_some(ViewCategory::TwoD),
            has_3d.then_some(ViewCategory::ThreeD),
            has_text.then_some(ViewCategory::Text),
            has_tensor.then_some(ViewCategory::Tensor),
        ]
        .iter()
        .filter_map(|cat| *cat)
        .collect()
    }
}

pub struct SceneQuery<'s> {
    pub obj_paths: &'s IntSet<ObjPath>,
    pub timeline: Timeline,
    pub time_query: TimeQuery<i64>,

    /// Controls what objects are visible
    pub obj_props: &'s ObjectsProperties,
}

impl<'s> SceneQuery<'s> {
    pub(crate) fn query(&self, ctx: &mut crate::misc::ViewerContext<'_>) -> Scene {
        crate::profile_function!();
        let mut scene = Scene::default();
        scene.two_d.load_objects(ctx, self);
        scene.three_d.load_objects(ctx, self);
        scene.text.load_objects(ctx, self);
        scene.tensor.load_objects(ctx, self);
        scene
    }

    /// Given a list of `ObjectType`s, this will return all relevant `ObjStore`s that should be
    /// queried for datapoints.
    ///
    /// An `ObjStore` is considered relevant if it contains at least one of the types that we
    /// are looking for, and is currently visible according to the state of the blueprint.
    pub(crate) fn iter_object_stores<'a>(
        &'a self,
        log_db: &'a LogDb,
        obj_types: &'a [ObjectType],
    ) -> impl Iterator<Item = (ObjectType, &ObjPath, &ObjStore<i64>)> + 'a {
        // For the appropriate timeline store...
        log_db
            .obj_db
            .store
            .get(&self.timeline)
            .into_iter()
            .flat_map(|timeline_store| {
                // ...and for all visible object paths within that timeline store...
                self.obj_paths
                    .iter()
                    .filter(|obj_path| self.obj_props.get(obj_path).visible)
                    .filter_map(|obj_path| {
                        // ...whose datatypes are registered...
                        let obj_type = log_db.obj_db.types.get(obj_path.obj_type_path());
                        obj_type
                            .and_then(|obj_type| {
                                // ...and whose datatypes we care about...
                                obj_types.contains(obj_type).then(|| {
                                    // ...then return the actual object store!
                                    timeline_store
                                        .get(obj_path)
                                        .map(|obj_store| (*obj_type, obj_path, obj_store))
                                })
                            })
                            .flatten()
                    })
            })
    }

    /// Given a [`FieldName`], this will return all relevant [`ObjStore`]s that contain
    /// the specified field.
    ///
    /// An [`ObjStore`] is considered relevant if it is the nearest ancestor
    /// that contains that field for some visible object in the space.
    ///
    /// An object is considered its own (nearest) ancestor.
    pub(crate) fn iter_ancestor_meta_field<'a>(
        &'a self,
        log_db: &'a LogDb,
        field_name: &'a FieldName,
    ) -> impl Iterator<Item = (ObjPath, &FieldStore<i64>)> + 'a {
        let mut visited = IntSet::<ObjPath>::default();
        let mut found_fields = HashMap::<ObjPath, &FieldStore<i64>>::default();
        for obj_path in self
            .obj_paths
            .iter()
            .filter(|obj_path| self.obj_props.get(obj_path).visible)
        {
            let mut next_parent = Some(obj_path.clone());
            while let Some(parent) = next_parent {
                // If we've visited this parent before it's safe to break early.
                // All of it's parents have have also been visited.
                if !visited.insert(parent.clone()) {
                    break;
                }

                match found_fields.entry(parent.clone()) {
                    // If we've hit this path before and found a match, we can also break.
                    // This should not actually get hit due to the above early-exit.
                    std::collections::hash_map::Entry::Occupied(_) => break,
                    // Otherwise check the obj_store for the field.
                    // If we find one, insert it and then we can break.
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        if log_db
                            .obj_db
                            .store
                            .get(&self.timeline)
                            .and_then(|timeline_store| timeline_store.get(&parent))
                            .and_then(|obj_store| obj_store.get(field_name))
                            .map(|field_store| entry.insert(field_store))
                            .is_some()
                        {
                            break;
                        }
                    }
                }
                // Finally recurse to the next parent up the path
                // TODO(jleibs): this is somewhat expensive as it needs to re-hash the object path
                // given ObjPathImpl is already an Arc, consider pre-computing and storing parents
                // for faster iteration.
                next_parent = parent.parent();
            }
        }
        found_fields.into_iter()
    }
}
