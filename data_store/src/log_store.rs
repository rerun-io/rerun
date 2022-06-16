use nohash_hasher::{IntMap, IntSet};

use log_types::{
    Data, DataMsg, DataPath, DataVec, LoggedData, ObjPath, ObjPathHash, TimeSource, TimeType,
};

use crate::{ArcBatch, Batch, TypePathDataStore};

#[derive(Default)]
pub struct LogDataStore {
    store_from_time_source: IntMap<TimeSource, (TimeType, TypePathDataStore<i64>)>,
    obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,
    /// To avoid doing double-work filling in [`Self::obj_path_from_hash`].
    regiestered_batch_paths: IntMap<ObjPathHash, IntSet<u64>>,
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
        crate::profile_function!();

        let mut batcher = Batcher::default();

        for (time_source, time_value) in &data_msg.time_point.0 {
            let time = time_value.as_i64();
            let id = data_msg.id;

            let DataPath {
                obj_path: op,
                field_name: fname,
            } = data_msg.data_path.clone();

            match &data_msg.data {
                LoggedData::Single(data) => {
                    self.obj_path_from_hash
                        .entry(*op.hash())
                        .or_insert_with(|| op.clone());

                    let store = self.entry(time_source, time_value.typ());
                    log_types::data_map!(data.clone(), |data| store
                        .insert_individual(op, fname, time, id, data))
                }
                LoggedData::Batch { indices, data } => {
                    log_types::data_vec_map!(data, |vec| {
                        let batch = batcher.batch(indices, vec);
                        self.register_batch_obj_paths(data_msg, batch.indices());
                        let store = self.entry(time_source, time_value.typ());
                        store.insert_batch(&op, fname, time, id, batch)
                    })
                }
                LoggedData::BatchSplat(data) => {
                    let store = self.entry(time_source, time_value.typ());
                    log_types::data_map!(data.clone(), |data| store
                        .insert_batch_splat(op, fname, time, id, data))
                }
            }?;
        }

        Ok(())
    }

    #[inline(never)]
    fn register_batch_obj_paths(
        &mut self,
        data_msg: &DataMsg,
        indices: std::slice::Iter<'_, log_types::IndexKey>,
    ) {
        crate::profile_function!();
        let obj_path = data_msg.data_path.obj_path();

        let registered_suffixes = self
            .regiestered_batch_paths
            .entry(*data_msg.data_path.obj_path.hash())
            .or_default();

        for index_path_suffix in indices {
            if registered_suffixes.insert(index_path_suffix.hash64()) {
                // TODO: speed this up. A lot. Please.
                let (obj_type_path, index_path_prefix) =
                    obj_path.clone().into_type_path_and_index_path();
                let mut index_path = index_path_prefix.clone();
                index_path.replace_last_placeholder_with(index_path_suffix.index().clone());
                let obj_path = ObjPath::new(obj_type_path.clone(), index_path);
                self.obj_path_from_hash.insert(*obj_path.hash(), obj_path);
            }
        }
    }
}

/// Converts data to a batch ONCE, then reuses that batch for other time sources
#[derive(Default)]
struct Batcher {
    batch: Option<Box<dyn std::any::Any>>,
}

impl Batcher {
    pub fn batch<T: 'static + Clone>(
        &mut self,
        indices: &[log_types::Index],
        data: &[T],
    ) -> ArcBatch<T> {
        if let Some(batch) = &self.batch {
            batch.downcast_ref::<ArcBatch<T>>().unwrap().clone()
        } else {
            let batch = std::sync::Arc::new(Batch::new(indices, data));
            self.batch = Some(Box::new(batch.clone()));
            batch
        }
    }
}
