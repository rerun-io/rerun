use nohash_hasher::IntMap;

use log_types::{Data, LogMsg, TimeSource, TimeType};

use crate::TypePathDataStore;

#[derive(Default)]
pub struct LogDataStore(IntMap<TimeSource, (TimeType, TypePathDataStore<i64>)>);

impl LogDataStore {
    pub fn get(&self, time_source: &TimeSource) -> Option<(TimeType, &TypePathDataStore<i64>)> {
        self.0
            .get(time_source)
            .map(|(time_type, store)| (*time_type, store))
    }

    pub fn entry(
        &mut self,
        time_source: &TimeSource,
        time_type: TimeType,
    ) -> &mut TypePathDataStore<i64> {
        match self.0.entry(time_source.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                &mut entry.insert((time_type, TypePathDataStore::default())).1
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().0 != time_type {
                    tracing::warn!("Time source {time_source:?} has multiple time types");
                }
                &mut entry.into_mut().1
            }
        }
    }

    pub fn insert(&mut self, log_msg: &LogMsg) -> crate::Result<()> {
        let (type_path, index_path) = crate::into_type_path(log_msg.data_path.clone());

        for (time_source, time_value) in &log_msg.time_point.0 {
            let store = self.entry(time_source, time_value.typ());
            let type_path = type_path.clone();
            let index_path = index_path.clone();
            let time = time_value.as_i64();
            let id = log_msg.id;

            #[allow(clippy::match_same_arms)]
            match log_msg.data.clone() {
                Data::I32(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::F32(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }

                Data::Color(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }

                Data::Pos2(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::BBox2D(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::LineSegments2D(value) => {
                    store.insert_individual(type_path, index_path, time, value)
                }
                Data::Image(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }

                Data::Pos3(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::Vec3(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::Box3(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::Path3D(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::LineSegments3D(value) => {
                    store.insert_individual(type_path, index_path, time, value)
                }
                Data::Mesh3D(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
                Data::Camera(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }

                Data::Vecf32(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }

                Data::Space(value) => {
                    store.insert_individual(type_path, index_path, time, (id, value))
                }
            }?;
        }

        Ok(())
    }
}
