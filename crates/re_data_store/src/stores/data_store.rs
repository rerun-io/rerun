use nohash_hasher::IntMap;

use re_log_types::*;

use crate::{
    ArcBatch, BadBatchError, Batch, BatchOrSplat, FieldQueryOutput, Result, TimeQuery,
    TimelineStore,
};

/// Stores all timelines of all objects.
#[derive(Default)]
pub struct DataStore {
    /// We store a copy of the data for each timeline.
    store_from_timeline: IntMap<Timeline, (TimeType, TimelineStore<i64>)>,

    /// In many places we just store the hashes, so we need a way to translate back.
    obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,

    /// In many places we just store the hashes, so we need a way to translate back.
    index_from_hash: IntMap<IndexHash, Index>,
}

impl DataStore {
    pub fn get(&self, timeline: &Timeline) -> Option<&TimelineStore<i64>> {
        Some(&self.store_from_timeline.get(timeline)?.1)
    }

    #[inline]
    pub fn obj_path_from_hash(&self, obj_path_hash: &ObjPathHash) -> Option<&ObjPath> {
        self.obj_path_from_hash.get(obj_path_hash)
    }

    #[inline]
    pub fn index_from_hash(&self, index_hash: &IndexHash) -> Option<&Index> {
        self.index_from_hash.get(index_hash)
    }

    fn entry(&mut self, timeline: &Timeline, time_type: TimeType) -> &mut TimelineStore<i64> {
        match self.store_from_timeline.entry(*timeline) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                &mut entry.insert((time_type, TimelineStore::default())).1
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().0 != time_type {
                    re_log::warn!("Timeline {timeline:?} has multiple time types");
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
        timeline: &Timeline,
        time_query: &TimeQuery<i64>,
        data_path: &DataPath,
    ) -> Option<Result<FieldQueryOutput<i64>>> {
        let store = self.get(timeline)?;
        let obj_store = store.get(&data_path.obj_path)?;
        let field_store = obj_store.get(&data_path.field_name)?;
        Some(field_store.query_field_to_datavec(time_query, None))
    }

    pub fn insert_data_msg(&mut self, data_msg: &DataMsg) -> Result<()> {
        crate::profile_function!();

        let DataMsg {
            msg_id,
            time_point,
            data_path,
            data,
        } = data_msg;

        self.insert_data(*msg_id, time_point, data_path, data)
    }

    pub fn insert_data(
        &mut self,
        msg_id: MsgId,
        time_point: &TimePoint,
        data_path: &DataPath,
        data: &LoggedData,
    ) -> Result<()> {
        self.register_obj_path(&data_path.obj_path);

        // We de-duplicate batches so we don't create one per timeline:
        let batch = if let LoggedData::Batch { indices, data } = data {
            Some(re_log_types::data_vec_map!(data, |vec| {
                let hashed_indices = indices
                    .iter()
                    .map(|index| (IndexHash::hash(index), index))
                    .collect::<Vec<_>>();

                let batch = std::sync::Arc::new(
                    Batch::new(&hashed_indices, vec)
                        .map_err(|BadBatchError| crate::Error::BadBatch)?,
                );
                self.register_hashed_indices(&hashed_indices);
                TypeErasedBatch::new(batch)
            }))
        } else {
            None
        };

        for (timeline, time_int) in &time_point.0 {
            let store = self.entry(timeline, timeline.typ());

            insert_msg_into_timeline_store(
                store,
                data_path,
                msg_id,
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
    fn register_hashed_indices(
        &mut self,
        hashed_indices: &[(re_log_types::IndexHash, &re_log_types::Index)],
    ) {
        crate::profile_function!();
        for (hash, index) in hashed_indices {
            self.index_from_hash
                .entry(*hash)
                .or_insert_with(|| (*index).clone());
        }
    }
}

fn insert_msg_into_timeline_store(
    timeline_store: &mut TimelineStore<i64>,
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
