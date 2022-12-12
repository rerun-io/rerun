use ahash::HashMap;
use nohash_hasher::IntSet;

use re_data_store::{
    FieldName, FieldStore, InstanceIdHash, LogDb, ObjPath, ObjStore, ObjectsProperties, TimeInt,
    TimeQuery, Timeline,
};
use re_log_types::ObjectType;

use super::{space_view::ViewCategory, view_plot, view_spatial, view_tensor, view_text};

// ---

/// A fully self-contained scene, ready to be rendered as-is.
#[derive(Default)]
pub struct Scene {
    pub spatial: view_spatial::SceneSpatial,
    pub text: view_text::SceneText,
    pub tensor: view_tensor::SceneTensor,
    pub plot: view_plot::ScenePlot,
}

impl Scene {
    pub(crate) fn categories(&self) -> std::collections::BTreeSet<ViewCategory> {
        let Self {
            spatial,
            text,
            tensor,
            plot,
        } = self;

        let has_spatial = !spatial.is_empty();
        let has_text = !text.is_empty();
        let has_tensor = !tensor.is_empty();
        let has_plot = !plot.is_empty();

        [
            has_spatial.then_some(ViewCategory::Spatial),
            has_text.then_some(ViewCategory::Text),
            has_tensor.then_some(ViewCategory::Tensor),
            has_plot.then_some(ViewCategory::Plot),
        ]
        .iter()
        .filter_map(|cat| *cat)
        .collect()
    }
}

pub struct SceneQuery<'s> {
    pub obj_paths: &'s IntSet<ObjPath>,
    pub timeline: Timeline,
    pub latest_at: TimeInt,

    /// Controls what objects are visible
    pub obj_props: &'s ObjectsProperties,
}

impl<'s> SceneQuery<'s> {
    pub(crate) fn query(&self, ctx: &mut crate::misc::ViewerContext<'_>) -> Scene {
        crate::profile_function!();
        let mut scene = Scene::default();
        let Scene {
            spatial,
            text,
            tensor,
            plot,
        } = &mut scene;
        spatial.load_objects(ctx, self, InstanceIdHash::NONE);
        text.load_objects(ctx, self, &view_text::ViewTextFilters::default());
        tensor.load_objects(ctx, self);
        plot.load_objects(ctx, self);
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
    ) -> impl Iterator<Item = (ObjectType, &ObjPath, TimeQuery<i64>, &ObjStore<i64>)> + 'a {
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
                    .map(|obj_path| (obj_path, self.obj_props.get(obj_path)))
                    .filter(|(_obj_path, obj_props)| obj_props.visible)
                    .filter_map(|(obj_path, obj_props)| {
                        let visible_history = match self.timeline.typ() {
                            re_log_types::TimeType::Time => obj_props.visible_history.nanos,
                            re_log_types::TimeType::Sequence => obj_props.visible_history.sequences,
                        };

                        let latest_at = self.latest_at.as_i64();
                        let time_query = if visible_history == 0 {
                            TimeQuery::LatestAt(latest_at)
                        } else {
                            TimeQuery::Range(latest_at.saturating_sub(visible_history)..=latest_at)
                        };

                        // ...whose datatypes are registered...
                        let obj_type = log_db.obj_db.types.get(obj_path.obj_type_path());
                        obj_type
                            .and_then(|obj_type| {
                                // ...and whose datatypes we care about...
                                obj_types.contains(obj_type).then(|| {
                                    // ...then return the actual object store!
                                    timeline_store.get(obj_path).map(|obj_store| {
                                        (*obj_type, obj_path, time_query, obj_store)
                                    })
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
