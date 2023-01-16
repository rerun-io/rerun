use itertools::Itertools;
use nohash_hasher::IntMap;

use re_log_types::{
    BatchIndex, DataMsg, DataPath, FieldOrComponent, Index, IndexHash, LoggedData, MsgId,
    TimePoint, TimeType, Timeline,
};

use crate::{
    profile_scope, ArcBatch, BadBatchError, Batch, BatchOrSplat, FieldQueryOutput, Result,
    TimeQuery, TimelineStore,
};

/// Stores all timelines of all objects.
#[derive(Default)]
pub struct DataStore {
    /// We store a copy of the data for each timeline.
    store_from_timeline: IntMap<Timeline, (TimeType, TimelineStore<i64>)>,

    /// In many places we just store the hashes, so we need a way to translate back.
    index_from_hash: IntMap<IndexHash, Index>,
}

impl DataStore {
    pub fn get(&self, timeline: &Timeline) -> Option<&TimelineStore<i64>> {
        Some(&self.store_from_timeline.get(timeline)?.1)
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
        if let FieldOrComponent::Field(field) = data_path.field_name {
            let field_store = obj_store.get(&field)?;
            Some(field_store.query_field_to_datavec(time_query, None))
        } else {
            None
        }
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
        // We de-duplicate batches so we don't create one per timeline:
        let batch = if let LoggedData::Batch { indices, data } = data {
            Some(re_log_types::data_vec_map!(data, |vec| {
                let batch = match indices {
                    BatchIndex::SequentialIndex(sz) => {
                        // If the index is the wrong size, return a BadBatch erro
                        if *sz != data.len() {
                            return Err(crate::Error::BadBatch);
                        }

                        // Use the shared pre-hashed values to update the registration
                        let hashed_indices = crate::SharedSequentialIndex::hashes_up_to(*sz);
                        self.register_hashed_indices(
                            &hashed_indices.0[..*sz],
                            &hashed_indices.1[..*sz],
                        );

                        std::sync::Arc::new(
                            Batch::new_sequential(vec)
                                .map_err(|BadBatchError| crate::Error::BadBatch)?,
                        )
                    }
                    BatchIndex::FullIndex(indices) => {
                        let hashed_indices = indices.iter().map(IndexHash::hash).collect_vec();

                        self.register_hashed_indices(&hashed_indices, indices);

                        std::sync::Arc::new(
                            Batch::new_indexed(&hashed_indices, vec)
                                .map_err(|BadBatchError| crate::Error::BadBatch)?,
                        )
                    }
                };
                TypeErasedBatch::new(batch)
            }))
        } else {
            None
        };

        for (timeline, time_int) in time_point.iter() {
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

    #[inline(never)]
    fn register_hashed_indices(&mut self, hashed_indices: &[IndexHash], indices: &[Index]) {
        crate::profile_function!();
        for (hash, index) in std::iter::zip(hashed_indices, indices) {
            self.index_from_hash
                .entry(*hash)
                .or_insert_with(|| index.clone());
        }
    }

    pub fn purge_everything_but(&mut self, keep_msg_ids: &ahash::HashSet<MsgId>) {
        crate::profile_function!();
        let Self {
            store_from_timeline,
            index_from_hash: _,
        } = self;
        for (timeline, (_, timeline_store)) in store_from_timeline {
            profile_scope!("purge_timeline", timeline.name().as_str());
            _ = timeline; // silence unused-variable warning on wasm
            timeline_store.purge_everything_but(keep_msg_ids);
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

    let FieldOrComponent::Field(field_name) = field_name
    else {
        return Err(crate::Error::WrongFieldType)
    };

    match data {
        LoggedData::Null(data_type) => {
            re_log_types::data_type_map_none!(data_type, |data_none| timeline_store
                .insert_mono(obj_path, field_name, time_i64, msg_id, data_none))
        }
        LoggedData::Single(data) => {
            re_log_types::data_map!(data.clone(), |data| timeline_store.insert_mono(
                obj_path,
                field_name,
                time_i64,
                msg_id,
                Some(data)
            ))
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
