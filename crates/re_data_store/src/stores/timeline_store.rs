use nohash_hasher::IntMap;

use re_log_types::{DataTrait, FieldName, MsgId, ObjPath};

use crate::{BatchOrSplat, ObjStore, Result};

/// Stores all objects for a specific timeline.
pub struct TimelineStore<Time> {
    // There is room for optimization here!
    // A lot of objects will share the same `ObjectType`,
    // and will therefore have the same `ObjStore` implementation (mono vs multi).
    // Thus we can get a nice speedup by having just one `ObjStore` per `ObjectType`
    // and then indexing on `IndexPath` in the `ObjStore`.
    // It adds some complexity though, so we will wait to cross that bridge until we need to.
    objects: IntMap<ObjPath, ObjStore<Time>>,
}

impl<Time> Default for TimelineStore<Time> {
    fn default() -> Self {
        Self {
            objects: Default::default(),
        }
    }
}

impl<Time: 'static + Copy + Ord> TimelineStore<Time> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&ObjPath, &ObjStore<Time>)> {
        self.objects.iter()
    }

    pub fn get(&self, obj_path: &ObjPath) -> Option<&ObjStore<Time>> {
        self.objects.get(obj_path)
    }

    pub fn insert_mono<T: DataTrait>(
        &mut self,
        obj_path: ObjPath,
        field_name: FieldName,
        time: Time,
        msg_id: MsgId,
        value: Option<T>,
    ) -> Result<()> {
        self.objects
            .entry(obj_path)
            .or_default()
            .insert_mono(field_name, time, msg_id, value)
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

    pub fn retain(
        &mut self,
        keep_msg_ids: Option<&ahash::HashSet<MsgId>>,
        drop_msg_ids: Option<&ahash::HashSet<MsgId>>,
    ) {
        let Self { objects } = self;
        for obj_store in objects.values_mut() {
            obj_store.retain(keep_msg_ids, drop_msg_ids);
        }
    }
}
