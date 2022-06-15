use nohash_hasher::IntMap;

use log_types::{
    Data, DataMsg, DataPath, DataVec, IndexKey, LoggedData, ObjPath, ObjPathHash, TimeSource,
    TimeType,
};

use crate::{Batch, TypePathDataStore};

#[derive(Default)]
pub struct LogDataStore {
    store_from_time_source: IntMap<TimeSource, (TimeType, TypePathDataStore<i64>)>,
    obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,
}

impl LogDataStore {
    pub fn get(&self, time_source: &TimeSource) -> Option<(TimeType, &TypePathDataStore<i64>)> {
        self.store_from_time_source
            .get(time_source)
            .map(|(time_type, store)| (*time_type, store))
    }

    #[inline]
    pub fn obj_path_from_hash(&self, obj_path_hash: &ObjPathHash) -> Option<&ObjPath> {
        self.obj_path_from_hash.get(obj_path_hash)
    }

    pub fn entry(
        &mut self,
        time_source: &TimeSource,
        time_type: TimeType,
    ) -> &mut TypePathDataStore<i64> {
        match self.store_from_time_source.entry(*time_source) {
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
        self.register_obj_path(data_msg);

        let mut batcher = Batcher::default();

        for (time_source, time_value) in &data_msg.time_point.0 {
            let store = self.entry(time_source, time_value.typ());
            let time = time_value.as_i64();
            let id = data_msg.id;

            let DataPath {
                obj_path: op,
                field_name: fname,
            } = data_msg.data_path.clone();

            match data_msg.data.clone() {
                LoggedData::Batch { indices, data } => {
                    log_types::data_vec_map!(data, |vec| {
                        let batch = batcher.batch(indices, vec);
                        store.insert_batch(&op, fname, time, id, batch)
                    })
                }
                LoggedData::Single(data) => {
                    log_types::data_map!(data, |data| store
                        .insert_individual(op, fname, time, id, data))
                }
            }?;
        }

        Ok(())
    }

    fn register_obj_path(&mut self, data_msg: &DataMsg) {
        let obj_path = data_msg.data_path.obj_path();

        match &data_msg.data {
            LoggedData::Batch { indices, .. } => {
                for index_path_suffix in indices {
                    crate::profile_scope!("Register batch obj paths");
                    let (obj_type_path, index_path_prefix) =
                        obj_path.clone().into_type_path_and_index_path();
                    // TODO: speed this up. A lot. Please.
                    let mut index_path = index_path_prefix.clone();
                    index_path.replace_last_placeholder_with(index_path_suffix.clone().into());
                    let obj_path = ObjPath::new(obj_type_path.clone(), index_path);
                    self.obj_path_from_hash.insert(*obj_path.hash(), obj_path);
                }
            }
            LoggedData::Single(_) => {
                self.obj_path_from_hash
                    .entry(*obj_path.hash())
                    .or_insert_with(|| obj_path.clone());
            }
        }
    }
}

fn batch<T>(indices: Vec<log_types::Index>, data: Vec<T>) -> Batch<T> {
    assert_eq!(indices.len(), data.len()); // TODO: return Result instead
    std::sync::Arc::new(
        itertools::izip!(indices, data)
            .map(|(index, value)| (IndexKey::new(index), value))
            .collect(),
    )
}

/// Converts data to a batch ONCE, then reuses that batch for other time sources
#[derive(Default)]
struct Batcher {
    batch: Option<Box<dyn std::any::Any>>,
}

impl Batcher {
    pub fn batch<T: 'static>(&mut self, indices: Vec<log_types::Index>, data: Vec<T>) -> Batch<T> {
        if let Some(batch) = &self.batch {
            batch.downcast_ref::<Batch<T>>().unwrap().clone()
        } else {
            let batch = batch(indices, data);
            self.batch = Some(Box::new(batch.clone()));
            batch
        }
    }
}
