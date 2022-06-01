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
        for (time_source, time_value) in &log_msg.time_point.0 {
            let store = self.entry(time_source, time_value.typ());
            let dp = log_msg.data_path.clone();
            let time = time_value.as_i64();
            let id = log_msg.id;

            #[allow(clippy::match_same_arms)]
            match log_msg.data.clone() {
                Data::I32(data) => store.insert_individual(dp, time, id, data),
                Data::F32(data) => store.insert_individual(dp, time, id, data),

                Data::Color(data) => store.insert_individual(dp, time, id, data),

                Data::Pos2(data) => store.insert_individual(dp, time, id, data),
                Data::BBox2D(data) => store.insert_individual(dp, time, id, data),
                Data::LineSegments2D(data) => store.insert_individual(dp, time, id, data),
                Data::Image(data) => store.insert_individual(dp, time, id, data),

                Data::Pos3(data) => store.insert_individual(dp, time, id, data),
                Data::Vec3(data) => store.insert_individual(dp, time, id, data),
                Data::Box3(data) => store.insert_individual(dp, time, id, data),
                Data::Path3D(data) => store.insert_individual(dp, time, id, data),
                Data::LineSegments3D(data) => store.insert_individual(dp, time, id, data),
                Data::Mesh3D(data) => store.insert_individual(dp, time, id, data),
                Data::Camera(data) => store.insert_individual(dp, time, id, data),

                Data::Vecf32(data) => store.insert_individual(dp, time, id, data),

                Data::Space(data) => store.insert_individual(dp, time, id, data),
            }?;
        }

        Ok(())
    }
}
