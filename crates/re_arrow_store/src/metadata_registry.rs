use std::collections::BTreeMap;

use nohash_hasher::IntMap;
use re_log_types::{EntityPathHash, RowId, TimeInt, TimePoint, Timeline};
use re_types_core::{ComponentName, SizeBytes};

// ---

// TODO: explain why these don't use the public view traits

// TODO: need to add all new registries to stats btw

// TODO: this really is our most barebone subscriber
/// Keeps track of arbitrary per-row metadata.
#[derive(Debug, Clone)]
pub struct MetadataRegistry<T: Clone> {
    pub registry: BTreeMap<RowId, T>,

    /// Cached heap size, because the registry gets very, very large.
    pub heap_size_bytes: u64,
}

impl Default for MetadataRegistry<TimePoint> {
    fn default() -> Self {
        let mut this = Self {
            registry: Default::default(),
            heap_size_bytes: 0,
        };
        this.heap_size_bytes = this.heap_size_bytes(); // likely zero, just future proofing
        this
    }
}

impl<T: Clone> std::ops::Deref for MetadataRegistry<T> {
    type Target = BTreeMap<RowId, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.registry
    }
}

impl<T: Clone> std::ops::DerefMut for MetadataRegistry<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.registry
    }
}

impl MetadataRegistry<TimePoint> {
    pub fn upsert(&mut self, row_id: RowId, timepoint: TimePoint) {
        let mut added_size_bytes = 0;

        // This is valuable information even for a timeless timepoint!
        match self.entry(row_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                // NOTE: In a map, thus on the heap!
                added_size_bytes += row_id.total_size_bytes();
                added_size_bytes += timepoint.total_size_bytes();
                entry.insert(timepoint);
            }
            // NOTE: When saving and loading data from disk, it's very possible that we try to
            // insert data for a single `RowId` in multiple calls (buckets are per-timeline, so a
            // single `RowId` can get spread across multiple buckets)!
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                for (timeline, time) in timepoint {
                    if let Some(old_time) = entry.insert(timeline, time) {
                        if old_time != time {
                            re_log::error!(%row_id, ?timeline, old_time = ?old_time, new_time = ?time, "detected re-used `RowId/Timeline` pair, this is illegal and will lead to undefined behavior in the datastore");
                            debug_assert!(false, "detected re-used `RowId/Timeline`");
                        }
                    } else {
                        // NOTE: In a map, thus on the heap!
                        added_size_bytes += timeline.total_size_bytes();
                        added_size_bytes += time.as_i64().total_size_bytes();
                    }
                }
            }
        }

        self.heap_size_bytes += added_size_bytes;
    }
}

// ---

// TODO: registry is a horrible name, let's call these what they are... secondary indices

// TODO: this really is our most barebone subscriber
/// Keeps track of arbitrary per-row metadata.
#[derive(Debug, Clone)]
pub struct Registry<K, T> {
    pub registry: BTreeMap<K, T>,

    /// Cached heap size, because the registry gets very, very large.
    pub heap_size_bytes: u64,
}

impl<K, T> std::ops::Deref for Registry<K, T> {
    type Target = BTreeMap<K, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.registry
    }
}

impl<K, T> std::ops::DerefMut for Registry<K, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.registry
    }
}

impl<K, T> SizeBytes for Registry<K, T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.heap_size_bytes
    }
}

impl<K, T> Default for Registry<K, T> {
    fn default() -> Self {
        let mut this = Self {
            registry: Default::default(),
            heap_size_bytes: 0,
        };
        this.heap_size_bytes = this.heap_size_bytes(); // likely zero, just future proofing
        this
    }
}

impl Registry<RowId, TimePoint> {
    /// Returns `true` iff the `RowId` is new.
    pub fn add(&mut self, row_id: RowId, timepoint: TimePoint) -> bool {
        let mut is_new = false;
        let mut added_size_bytes = 0;

        // This is valuable information even for a timeless timepoint!
        match self.entry(row_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                // NOTE: In a map, thus on the heap!
                added_size_bytes += row_id.total_size_bytes();
                added_size_bytes += timepoint.total_size_bytes();
                entry.insert(timepoint);
                is_new = true;
            }
            // NOTE: When saving and loading data from disk, it's very possible that we try to
            // insert data for a single `RowId` in multiple calls (buckets are per-timeline, so a
            // single `RowId` can get spread across multiple buckets)!
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                for (timeline, time) in timepoint {
                    if let Some(old_time) = entry.insert(timeline, time) {
                        if old_time != time {
                            re_log::error!(%row_id, ?timeline, old_time = ?old_time, new_time = ?time, "detected re-used `RowId/Timeline` pair, this is illegal and will lead to undefined behavior in the datastore");
                            debug_assert!(false, "detected re-used `RowId/Timeline`");
                        }
                    } else {
                        // NOTE: In a map, thus on the heap!
                        added_size_bytes += timeline.total_size_bytes();
                        added_size_bytes += time.as_i64().total_size_bytes();
                    }
                }
            }
        }

        self.heap_size_bytes += added_size_bytes;

        is_new
    }

    // TODO
    // /// Returns `true` iff this removed the last-standing `RowId`.
    // pub fn remove(&mut self, row_id: RowId) -> bool {
    //
    // }
}

impl Registry<EntityPathHash, ()> {
    /// Returns `true` iff the `EntityPathHash` is new.
    pub fn add(&mut self, ent_path_hash: EntityPathHash) -> bool {
        let mut is_new = false;
        let mut added_size_bytes = 0;

        if self.insert(ent_path_hash, ()).is_none() {
            // NOTE: In a map, thus on the heap!
            added_size_bytes += ent_path_hash.total_size_bytes();
            is_new = true;
        }

        self.heap_size_bytes += added_size_bytes;

        is_new
    }
}

impl Registry<(EntityPathHash, ComponentName), IntMap<Timeline, TimeInt>> {
    /// Returns `true`
    pub fn add(
        &mut self,
        ent_path_hash: EntityPathHash,
        comp_name: ComponentName,
        (timeline, time): (Timeline, TimeInt),
    ) -> bool {
        let mut is_new = false;
        let mut added_size_bytes = 0;

        // TODO: if timeless, this will be empty, but it will exist!!

        // This is valuable information even for a timeless timepoint!
        match self.entry((ent_path_hash, comp_name)) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                // NOTE: In a map, thus on the heap!
                added_size_bytes += ent_path_hash.total_size_bytes();
                added_size_bytes += time.total_size_bytes();
                entry.insert(Default::default()).insert(timeline, time);
                is_new = true;
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                if entry.insert(timeline, time).is_none() {
                    // NOTE: In a map, thus on the heap!
                    added_size_bytes += timeline.total_size_bytes();
                    added_size_bytes += time.as_i64().total_size_bytes();
                    is_new = true;
                }
            }
        }

        self.heap_size_bytes += added_size_bytes;

        is_new
    }
}
