use nohash_hasher::IntMap;

use re_log_types::{
    DataMsg, DataPath, DataTrait, FieldName, Index, IndexHash, LoggedData, MsgId, ObjPath,
    ObjPathHash, TimeSource, TimeType,
};

use crate::{ArcBatch, Batch, BatchOrSplat, ObjStore, Result};

/// Stores all objects for a specific timeline.
pub struct FullStore<Time> {
    // There is room for optimization here!
    // A lot of objects will share the same `ObjectType`,
    // and will therefore have the same `ObjStore` implementation (mono vs multi).
    // Thus we can get a nice speedup by having just one `ObjStore` per `ObjectType`
    // and then indexing on `IndexPath` in the `ObjStore`.
    // It adds some complexity though, so we will wait to cross that bridge until we need to.
    objects: IntMap<ObjPath, ObjStore<Time>>,
}

impl<Time> Default for FullStore<Time> {
    fn default() -> Self {
        Self {
            objects: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord> FullStore<Time> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&ObjPath, &ObjStore<Time>)> {
        self.objects.iter()
    }

    pub fn get(&self, obj_path: &ObjPath) -> Option<&ObjStore<Time>> {
        self.objects.get(obj_path)
    }

    pub fn insert_individual<T: DataTrait>(
        &mut self,
        obj_path: ObjPath,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        value: T,
    ) -> Result<()> {
        self.objects
            .entry(obj_path)
            .or_default()
            .insert_individual(field_name, time, msg_id, value)
    }

    pub fn insert_batch<T: DataTrait>(
        &mut self,
        obj_path: ObjPath,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        batch: BatchOrSplat<T>,
    ) -> Result<()> {
        self.objects
            .entry(obj_path)
            .or_default()
            .insert_batch(field_name, time, msg_id, batch)
    }
}

/// Stores all timelines of all objects.
#[derive(Default)]
pub struct LogDataStore {
    /// We store a copy of the data for each time source.
    store_from_time_source: IntMap<TimeSource, (TimeType, FullStore<i64>)>,

    /// In many places we just store the hashes, so we need a way to translate back.
    obj_path_from_hash: IntMap<ObjPathHash, ObjPath>,

    /// In many places we just store the hashes, so we need a way to translate back.
    index_from_hash: IntMap<IndexHash, Index>,
}

impl LogDataStore {
    pub fn get(&self, time_source: &TimeSource) -> Option<&FullStore<i64>> {
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

    pub fn entry(&mut self, time_source: &TimeSource, time_type: TimeType) -> &mut FullStore<i64> {
        match self.store_from_time_source.entry(*time_source) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                &mut entry.insert((time_type, FullStore::default())).1
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().0 != time_type {
                    tracing::warn!("Time source {time_source:?} has multiple time types");
                }
                &mut entry.into_mut().1
            }
        }
    }

    pub fn insert(&mut self, data_msg: &DataMsg) -> Result<()> {
        crate::profile_function!();

        let mut batcher = Batcher::default();

        for (time_source, time_int) in &data_msg.time_point.0 {
            let time_i64 = time_int.as_i64();
            let msg_id = data_msg.msg_id;

            let DataPath {
                obj_path,
                field_name,
            } = data_msg.data_path.clone();

            self.register_obj_path(&obj_path);

            match &data_msg.data {
                LoggedData::Single(data) => {
                    let store = self.entry(time_source, time_source.typ());
                    re_log_types::data_map!(data.clone(), |data| store
                        .insert_individual(obj_path, field_name, time_i64, msg_id, data))
                }
                LoggedData::Batch { indices, data } => {
                    re_log_types::data_vec_map!(data, |vec| {
                        let batch = batcher.batch(indices, vec);
                        self.register_batch_indices(batch.as_ref());
                        let store = self.entry(time_source, time_source.typ());
                        store.insert_batch(
                            obj_path,
                            field_name,
                            time_i64,
                            msg_id,
                            BatchOrSplat::Batch(batch),
                        )
                    })
                }
                LoggedData::BatchSplat(data) => {
                    let store = self.entry(time_source, time_source.typ());
                    re_log_types::data_map!(data.clone(), |data| {
                        let batch = crate::BatchOrSplat::Splat(data);
                        store.insert_batch(obj_path, field_name, time_i64, msg_id, batch)
                    })
                }
            }?;
        }

        Ok(())
    }

    fn register_obj_path(&mut self, obj_path: &ObjPath) {
        let hash = *obj_path.hash();
        #[allow(clippy::map_entry)]
        // False positive: we want to avoid cloning the path until we really need to.
        if !self.obj_path_from_hash.contains_key(&hash) {
            self.obj_path_from_hash.insert(hash, obj_path.clone());
        }
    }

    #[inline(never)]
    fn register_batch_indices<T>(&mut self, batch: &Batch<T>) {
        crate::profile_function!();
        for (hash, index) in batch.indices() {
            if !self.index_from_hash.contains_key(hash) {
                self.index_from_hash.insert(*hash, index.clone());
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
        indices: &[re_log_types::Index],
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
