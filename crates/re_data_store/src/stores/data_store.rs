use nohash_hasher::IntMap;

use re_log_types::*;

use crate::{
    ArcBatch, BadBatchError, Batch, BatchOrSplat, FieldQueryOutput, Result, TimeLineStore,
    TimeQuery,
};

/// Stores all timelines of all objects.
#[derive(Default)]
pub struct DataStore {
    /// We store a copy of the data for each time source.
    store_from_time_source: IntMap<TimeSource, (TimeType, TimeLineStore<i64>)>,

    /// In many places we just store the hashes, so we need a way to translate back.
    obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,

    /// In many places we just store the hashes, so we need a way to translate back.
    index_from_hash: IntMap<IndexHash, Index>,
}

impl DataStore {
    pub fn get(&self, time_source: &TimeSource) -> Option<&TimeLineStore<i64>> {
        Some(&self.store_from_time_source.get(time_source)?.1)
    }

    #[inline]
    pub fn obj_path_from_hash(&self, obj_path_hash: &ObjPathHash) -> Option<&ObjPath> {
        self.obj_path_from_hash.get(obj_path_hash)
    }

    #[inline]
    pub fn index_from_hash(&self, index_hash: &IndexHash) -> Option<&Index> {
        self.index_from_hash.get(index_hash)
    }

    fn entry(&mut self, time_source: &TimeSource, time_type: TimeType) -> &mut TimeLineStore<i64> {
        match self.store_from_time_source.entry(*time_source) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                &mut entry.insert((time_type, TimeLineStore::default())).1
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().0 != time_type {
                    re_log::warn!("Time source {time_source:?} has multiple time types");
                }
                &mut entry.into_mut().1
            }
        }
    }

    /// Query a specific data path.
    ///
    /// Return `None` if there were no such timeline, object, or field.
    pub fn query_data_path(
        &self,
        time_source: &TimeSource,
        time_query: &TimeQuery<i64>,
        data_path: &DataPath,
    ) -> Option<Result<FieldQueryOutput<i64>>> {
        let store = self.get(time_source)?;
        let obj_store = store.get(&data_path.obj_path)?;
        let field_store = obj_store.get(&data_path.field_name)?;
        Some(field_store.query_field_to_datavec(time_query, None))
    }

    pub fn insert(&mut self, data_msg: &DataMsg) -> Result<()> {
        crate::profile_function!();

        let DataMsg {
            msg_id,
            time_point,
            data_path,
            data,
        } = data_msg;

        self.register_obj_path(&data_path.obj_path);

        // We de-duplicate batches so we don't create one per timeline:
        let batch = if let LoggedData::Batch { indices, data } = data {
            Some(re_log_types::data_vec_map!(data, |vec| {
                let batch = std::sync::Arc::new(
                    Batch::new(indices, vec).map_err(|BadBatchError| crate::Error::BadBatch)?,
                );
                self.register_batch_indices(batch.as_ref());
                TypeErasedBatch::new(batch)
            }))
        } else {
            None
        };

        for (time_source, time_int) in &time_point.0 {
            let store = self.entry(time_source, time_source.typ());

            insert_msg_into_timeline_store(
                store,
                data_path,
                *msg_id,
                time_int.as_i64(),
                data,
                batch.as_ref(),
            )?;
        }

        Ok(())
    }

    fn register_obj_path(&mut self, obj_path: &ObjPath) {
        self.obj_path_from_hash
            .entry(*obj_path.hash())
            .or_insert_with(|| obj_path.clone());
    }

    #[inline(never)]
    fn register_batch_indices<T>(&mut self, batch: &Batch<T>) {
        crate::profile_function!();
        for (hash, index) in batch.indices() {
            self.index_from_hash
                .entry(*hash)
                .or_insert_with(|| index.clone());
        }
    }
}

fn insert_msg_into_timeline_store(
    timeline_store: &mut TimeLineStore<i64>,
    data_path: &DataPath,
    msg_id: MsgId,
    time_i64: i64,
    data: &LoggedData,
    batch: Option<&TypeErasedBatch>,
) -> Result<()> {
    let DataPath {
        obj_path,
        field_name,
    } = data_path.clone();

    match data {
        LoggedData::Single(data) => {
            re_log_types::data_map!(data.clone(), |data| timeline_store
                .insert_mono(obj_path, field_name, time_i64, msg_id, data))
        }
        LoggedData::Batch { data, .. } => {
            re_log_types::data_vec_map!(data, |vec| {
                let batch = batch.as_ref().unwrap().downcast(vec);
                timeline_store.insert_batch(
                    obj_path,
                    field_name,
                    time_i64,
                    msg_id,
                    BatchOrSplat::Batch(batch),
                )
            })
        }
        LoggedData::BatchSplat(data) => {
            re_log_types::data_map!(data.clone(), |data| {
                let batch = crate::BatchOrSplat::Splat(data);
                timeline_store.insert_batch(obj_path, field_name, time_i64, msg_id, batch)
            })
        }
    }
}

struct TypeErasedBatch(Box<dyn std::any::Any>);

impl TypeErasedBatch {
    fn new<T: 'static>(batch: ArcBatch<T>) -> Self {
        Self(Box::new(batch))
    }

    fn downcast<T: 'static>(&self, _only_used_for_type_inference: &[T]) -> ArcBatch<T> {
        self.0.downcast_ref::<ArcBatch<T>>().unwrap().clone()
    }
}
