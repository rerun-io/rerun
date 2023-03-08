use std::{borrow::Cow, collections::BTreeMap};

use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_arrow_store::{DataStoreConfig, GarbageCollectionTarget, TimeInt};
use re_log_types::{
    component_types::InstanceKey,
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    msg_bundle::{Component as _, ComponentBundle, MsgBundle},
    ArrowMsg, BeginRecordingMsg, ComponentPath, EntityPath, EntityPathHash, EntityPathOpMsg,
    LogMsg, MsgId, PathOp, RecordingId, RecordingInfo, TimePoint, Timeline,
};

use crate::{Error, TimesPerTimeline};

// ----------------------------------------------------------------------------

/// Stored entities with easy indexing of the paths.
pub struct EntityDb {
    /// In many places we just store the hashes, so we need a way to translate back.
    pub entity_path_from_hash: IntMap<EntityPathHash, EntityPath>,

    /// Used for time control
    pub times_per_timeline: TimesPerTimeline,

    /// A tree-view (split on path components) of the entities.
    pub tree: crate::EntityTree,

    /// Stores all components for all entities for all timelines.
    pub data_store: re_arrow_store::DataStore,
}

impl Default for EntityDb {
    fn default() -> Self {
        Self {
            entity_path_from_hash: Default::default(),
            times_per_timeline: Default::default(),
            tree: crate::EntityTree::root(),
            data_store: re_arrow_store::DataStore::new(
                InstanceKey::name(),
                DataStoreConfig {
                    component_bucket_nb_rows: 1,
                    index_bucket_nb_rows: 1,
                    component_bucket_size_bytes: 1024 * 1024, // 1 MiB
                    index_bucket_size_bytes: 1024,            // 1KiB
                    ..Default::default()
                },
            ),
        }
    }
}

impl EntityDb {
    #[inline]
    pub fn entity_path_from_hash(&self, entity_path_hash: &EntityPathHash) -> Option<&EntityPath> {
        self.entity_path_from_hash.get(entity_path_hash)
    }

    fn register_entity_path(&mut self, entity_path: &EntityPath) {
        self.entity_path_from_hash
            .entry(entity_path.hash())
            .or_insert_with(|| entity_path.clone());
    }

    fn try_add_arrow_data_msg(&mut self, msg: &ArrowMsg) -> Result<(), Error> {
        let msg_bundle = MsgBundle::try_from(msg).map_err(Error::MsgBundleError)?;

        for (&timeline, &time_int) in msg_bundle.time_point.iter() {
            self.times_per_timeline.insert(timeline, time_int);
        }

        self.register_entity_path(&msg_bundle.entity_path);

        for component in &msg_bundle.components {
            let component_path =
                ComponentPath::new(msg_bundle.entity_path.clone(), component.name());
            if component.name() == MsgId::name() {
                continue;
            }
            let pending_clears = self
                .tree
                .add_data_msg(&msg_bundle.time_point, &component_path);

            for (msg_id, time_point) in pending_clears {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let bundle =
                    ComponentBundle::new_empty(component.name(), component.data_type().clone());
                let msg_bundle = MsgBundle::new(
                    msg_id,
                    msg_bundle.entity_path.clone(),
                    time_point.clone(),
                    vec![bundle],
                );
                self.data_store.insert(&msg_bundle).ok();

                // Also update the tree with the clear-event
                self.tree.add_data_msg(&time_point, &component_path);
            }
        }

        self.data_store.insert(&msg_bundle).map_err(Into::into)
    }

    fn add_path_op(&mut self, msg_id: MsgId, time_point: &TimePoint, path_op: &PathOp) {
        let cleared_paths = self.tree.add_path_op(msg_id, time_point, path_op);

        for component_path in cleared_paths {
            if let Some(data_type) = self
                .data_store
                .lookup_data_type(&component_path.component_name)
            {
                // Create and insert an empty component into the arrow store
                // TODO(jleibs): Faster empty-array creation
                let bundle =
                    ComponentBundle::new_empty(component_path.component_name, data_type.clone());
                let msg_bundle = MsgBundle::new(
                    msg_id,
                    component_path.entity_path.clone(),
                    time_point.clone(),
                    vec![bundle],
                );
                self.data_store.insert(&msg_bundle).ok();
                // Also update the tree with the clear-event
                self.tree.add_data_msg(time_point, &component_path);
            }
        }
    }

    pub fn purge(
        &mut self,
        cutoff_times: &std::collections::BTreeMap<Timeline, TimeInt>,
        drop_msg_ids: &ahash::HashSet<MsgId>,
    ) {
        crate::profile_function!();

        let Self {
            entity_path_from_hash: _,
            times_per_timeline,
            tree,
            data_store: _, // purged before this function is called
        } = self;

        {
            crate::profile_scope!("times_per_timeline");
            times_per_timeline.purge(cutoff_times);
        }

        {
            crate::profile_scope!("tree");
            tree.purge(cutoff_times, drop_msg_ids);
        }
    }
}

// ----------------------------------------------------------------------------

/// A in-memory database built from a stream of [`LogMsg`]es.
#[derive(Default)]
pub struct LogDb {
    /// All the control messages (i.e. everything but the actual data / `ArrowMsg`), in ascending
    /// order according to their `MsgId`.
    ///
    /// Reminder: `MsgId`s are timestamped using the client's wall clock.
    control_messages: BTreeMap<MsgId, LogMsg>,

    /// Set by whomever created this [`LogDb`].
    pub data_source: Option<re_smart_channel::Source>,

    /// Comes in a special message, [`LogMsg::BeginRecordingMsg`].
    recording_info: Option<RecordingInfo>,

    /// Where we store the entities.
    pub entity_db: EntityDb,
}

impl LogDb {
    pub fn recording_info(&self) -> Option<&RecordingInfo> {
        self.recording_info.as_ref()
    }

    pub fn recording_id(&self) -> RecordingId {
        if let Some(info) = &self.recording_info {
            info.recording_id
        } else {
            RecordingId::ZERO
        }
    }

    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.times_per_timeline().timelines()
    }

    pub fn times_per_timeline(&self) -> &TimesPerTimeline {
        &self.entity_db.times_per_timeline
    }

    pub fn num_timeless_messages(&self) -> usize {
        self.entity_db.tree.num_timeless_messages()
    }

    pub fn is_empty(&self) -> bool {
        self.control_messages.is_empty()
    }

    pub fn add(&mut self, msg: LogMsg) -> Result<(), Error> {
        crate::profile_function!();

        let msg_id = match &msg {
            LogMsg::BeginRecordingMsg(msg) => {
                self.add_begin_recording_msg(msg);
                msg.msg_id
            }
            LogMsg::EntityPathOpMsg(msg) => {
                let EntityPathOpMsg {
                    msg_id,
                    time_point,
                    path_op,
                } = msg;
                self.entity_db.add_path_op(*msg_id, time_point, path_op);
                *msg_id
            }
            LogMsg::ArrowMsg(msg) => {
                self.entity_db.try_add_arrow_data_msg(msg)?;
                // Not a control message, return early.
                return Ok(());
            }
            LogMsg::Goodbye(msg_id) => *msg_id,
        };

        self.control_messages.insert(msg_id, msg);

        Ok(())
    }

    fn add_begin_recording_msg(&mut self, msg: &BeginRecordingMsg) {
        self.recording_info = Some(msg.info.clone());
    }

    pub fn len(&self) -> usize {
        self.control_messages.len()
        // TODO: add arrow DB "length" (?) (use DataStoreStats maybe?)
        // TODO: this wont make much sense with batches anyway
    }

    /// Returns all log messages in the database, in the order they arrived.
    ///
    // TODO: well... almost in the order they arrived.
    // TODO: really it's "insertion order"
    pub fn chronological_log_messages(&self) -> impl Iterator<Item = (TimePoint, Cow<'_, LogMsg>)> {
        // TODO: this will also fix save-to-selection which.. turns out it's been broken since the
        // switch to arrow.
        //
        // TODO: guess what, the timeless stuff doesn't even have a log time...

        // TODO: alright, so what if we introduced an actual timeline driven by the msg_id times?

        // TODO: beginrecording shouldn't really be a thing though, you should just ask the server
        // to register a new recording with the given metadata and get an ID back?
        // Or simpler: just generate a UUID on the client
        //
        // maybe that's a separate issue though?
        //
        // Now that I think of it, this is actually very much related to
        // https://github.com/rerun-io/rerun/issues/903
        //
        // Not only so-called chronological (insertion) order is problematic, but those things are
        // also stateful which makes matters worse (imagine if sql's `use DB` was stateful)
        // Also they live externally...
        //
        // Also: how we all of that fit into the general story of dumping the store into a native
        // file (i.e. dropping rrd?).
        //
        // Also: GC shouldn't even really be a thing though, the streaming system should just
        // forward to a file... and sure, that file might be the void / dev/null

        // let meta = self
        //     .chronological_message_ids
        //     .iter()
        //     .filter_map(|id| self.get_log_msg(id))
        //     .map(|msg| match msg {
        //         LogMsg::BeginRecordingMsg(inner) => (inner.msg_id, TimePoint::timeless(), msg),
        //         LogMsg::EntityPathOpMsg(inner) => (inner.msg_id, inner.time_point.clone(), msg),
        //         LogMsg::ArrowMsg(_) => {
        //             panic!("Arrow messages should never be stored in Viewer memory")
        //         }
        //         LogMsg::Goodbye(inner) => (*inner, TimePoint::timeless(), msg),
        //     })
        //     .map(|(id, tp, msg)| (id, tp, Cow::Borrowed(msg)));

        // TODO: not actually sure how any of this behaves after a GC pass? though at this point
        // the impacted msg ids should not be present in `chronological_message_ids` anymore so we
        // can filter that out

        let mut data: HashMap<MsgId, (TimePoint, _)> = self
            .entity_db
            .data_store
            .as_msg_bundles_xxx(MsgId::name())
            .filter_map(|msg_bundle| {
                let msg_id = msg_bundle.msg_id;
                let tp = msg_bundle.time_point.clone();

                // NOTE: Serialization shouldn't possibly be able to fail: this had to be
                // serialized as-is to be inserted into the store in the first place.
                // But.. you know.
                let msg: ArrowMsg = match msg_bundle.try_into() {
                    Ok(msg) => msg,
                    Err(err) => {
                        re_log::error_once!(
                            "Failed to serialize a reconstructed MsgBundle ({err}):\
                                something has gone seriously wrong"
                        );
                        return None;
                    }
                };

                Some((msg_id, (tp, Cow::Owned(LogMsg::ArrowMsg(msg)))))
            })
            .collect();

        let control = self
            .control_messages
            .iter()
            .filter_map(move |(msg_id, msg)| {
                Some(match msg {
                    LogMsg::BeginRecordingMsg(_) => (TimePoint::timeless(), msg),
                    LogMsg::EntityPathOpMsg(inner) => (inner.time_point.clone(), msg),
                    LogMsg::ArrowMsg(_) => return None,
                    LogMsg::Goodbye(_) => (TimePoint::timeless(), msg),
                })
            })
            .map(|(tp, msg)| (tp, Cow::Borrowed(msg)));

        // self.chronological_message_ids.iter().filter_map(move |id| {
        //     self.get_log_msg(id)
        //         .map(|(tp, msg)| (tp, Cow::Borrowed(msg)))
        //         .or(data.remove(id))
        // })

        // TODO: explain why we go through all this pain (and open a ticket once we've talked
        // it through): control vs. data messages, "chronological" order that the store knows
        // nothing of (and isn't chronological)

        // // TODO: we're going to need to talk about time...
        // // let mut arrow_msgs: IntMap<MsgId, MsgBundle> = Default::default();
        // // let data = self.entity_db.data_store.as_msg_bundles(MsgId::name());
        // // for data in data {
        // //     //
        // // }

        // let data = self
        //     .entity_db
        //     .data_store
        //     .as_msg_bundles(MsgId::name())
        //     .filter_map(|msg_bundle| {
        //         let msg_id = msg_bundle.msg_id;
        //         let tp = msg_bundle.time_point.clone();

        //         // NOTE: Serialization shouldn't possibly be able to fail: this had to be
        //         // serialized as-is to be inserted into the store in the first place.
        //         // But.. you know.
        //         let msg: ArrowMsg = match msg_bundle.try_into() {
        //             Ok(msg) => msg,
        //             Err(err) => {
        //                 re_log::error_once!(
        //                     "Failed to serialize a reconstructed MsgBundle ({err}):\
        //                         something has gone seriously wrong"
        //                 );
        //                 return None;
        //             }
        //         };

        //         Some((msg_id, tp, Cow::Owned(LogMsg::ArrowMsg(msg))))
        //     });

        // // TODO: or we pop from above rather than merging?

        // // TODO: at this point in particular it could get weird
        // // TODO: need to have a second look at the main PR too
        // // TODO: really need to get read of non-arrow messages
        // meta.merge_by(data, |meta, data| meta.0 <= data.0)
        //     .map(|(_, tp, msg)| (tp, msg))
    }

    // TODO
    pub fn get_log_msg(&self, msg_id: &MsgId) -> Option<(TimePoint, &LogMsg)> {
        self.log_messages.get(msg_id).and_then(|msg| {
            Some(match msg {
                LogMsg::BeginRecordingMsg(_) => (TimePoint::timeless(), msg),
                LogMsg::EntityPathOpMsg(inner) => (inner.time_point.clone(), msg),
                LogMsg::ArrowMsg(_) => return None,
                LogMsg::Goodbye(_) => (TimePoint::timeless(), msg),
            })
        })
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        crate::profile_function!();
        assert!((0.0..=1.0).contains(&fraction_to_purge));

        let drop_msg_ids = {
            let msg_id_chunks = self.entity_db.data_store.gc(
                GarbageCollectionTarget::DropAtLeastPercentage(fraction_to_purge as _),
                Timeline::log_time(),
                MsgId::name(),
            );

            msg_id_chunks
                .iter()
                .flat_map(|chunk| {
                    arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap()
                })
                // TODO: about that...
                .map(Option::unwrap) // MsgId is always present
                .collect::<ahash::HashSet<_>>()
        };

        let cutoff_times = self.entity_db.data_store.oldest_time_per_timeline();

        let Self {
            control_messages,
            chronological_message_ids,
            log_messages,
            data_source: _,
            recording_info: _,
            entity_db,
        } = self;

        {
            crate::profile_scope!("chronological_message_ids");
            chronological_message_ids.retain(|msg_id| !drop_msg_ids.contains(msg_id));
        }

        {
            crate::profile_scope!("log_messages");
            log_messages.retain(|msg_id, _| !drop_msg_ids.contains(msg_id));
        }

        entity_db.purge(&cutoff_times, &drop_msg_ids);
    }
}
