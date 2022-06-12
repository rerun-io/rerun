use nohash_hasher::IntMap;

use log_types::{Data, DataBatch, DataMsg, DataPath, IndexKey, ObjPath, TimeSource, TimeType};

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
        match self.0.entry(*time_source) {
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

    pub fn insert(&mut self, data_msg: &DataMsg) -> crate::Result<()> {
        for (time_source, time_value) in &data_msg.time_point.0 {
            let store = self.entry(time_source, time_value.typ());
            let time = time_value.as_i64();
            let id = data_msg.id;

            let DataPath {
                obj_path: op,
                field_name: fname,
            } = data_msg.data_path.clone();

            #[allow(clippy::match_same_arms)]
            match data_msg.data.clone() {
                Data::Batch { indices, data } => {
                    // TODO: reuse batch over time sources to save RAM
                    let (obj_type_path, index_path) = op.into_type_path_and_index_path();

                    match data {
                        DataBatch::Color(data) => {
                            assert_eq!(indices.len(), data.len());
                            let batch: crate::Batch<[u8; 4]> = std::sync::Arc::new(
                                indices
                                    .iter()
                                    .zip(data)
                                    .map(|(index, value)| (IndexKey::new(index.clone()), value))
                                    .collect(),
                            );
                            store.insert_batch(obj_type_path, index_path, fname, time, id, batch)
                        }
                        DataBatch::Pos3(data) => {
                            assert_eq!(indices.len(), data.len());
                            let batch: crate::Batch<[f32; 3]> = std::sync::Arc::new(
                                indices
                                    .iter()
                                    .zip(data)
                                    .map(|(index, value)| (IndexKey::new(index.clone()), value))
                                    .collect(),
                            );
                            store.insert_batch(obj_type_path, index_path, fname, time, id, batch)
                        }
                        DataBatch::Space(data) => {
                            assert_eq!(indices.len(), data.len());
                            let batch: crate::Batch<ObjPath> = std::sync::Arc::new(
                                indices
                                    .iter()
                                    .zip(data)
                                    .map(|(index, value)| (IndexKey::new(index.clone()), value))
                                    .collect(),
                            );
                            store.insert_batch(obj_type_path, index_path, fname, time, id, batch)
                        }
                    }
                }

                Data::I32(data) => store.insert_individual(op, fname, time, id, data),
                Data::F32(data) => store.insert_individual(op, fname, time, id, data),

                Data::Color(data) => store.insert_individual(op, fname, time, id, data),

                Data::Pos2(data) => store.insert_individual(op, fname, time, id, data),
                Data::BBox2D(data) => store.insert_individual(op, fname, time, id, data),
                Data::LineSegments2D(data) => store.insert_individual(op, fname, time, id, data),
                Data::Image(data) => store.insert_individual(op, fname, time, id, data),

                Data::Pos3(data) => store.insert_individual(op, fname, time, id, data),
                Data::Vec3(data) => store.insert_individual(op, fname, time, id, data),
                Data::Box3(data) => store.insert_individual(op, fname, time, id, data),
                Data::Path3D(data) => store.insert_individual(op, fname, time, id, data),
                Data::LineSegments3D(data) => store.insert_individual(op, fname, time, id, data),
                Data::Mesh3D(data) => store.insert_individual(op, fname, time, id, data),
                Data::Camera(data) => store.insert_individual(op, fname, time, id, data),

                Data::Vecf32(data) => store.insert_individual(op, fname, time, id, data),

                Data::Space(data) => store.insert_individual(op, fname, time, id, data),
            }?;
        }

        Ok(())
    }
}
