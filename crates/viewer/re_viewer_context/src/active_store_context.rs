use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, StoreId};

use crate::{Cache, CacheEntryAccess, StoreCache, StoreHub, TimeControl};

/// The current Blueprint and Recording being displayed by the viewer.
///
/// This is only constructed when the viewer is currently displaying a recording.
pub struct ActiveStoreContext<'a> {
    /// The current active blueprint.
    pub blueprint: &'a EntityDb,

    /// The default blueprint (i.e. the one logged from code), if any.
    pub default_blueprint: Option<&'a EntityDb>,

    /// The current active recording.
    ///
    /// If none is active, this will point to a dummy empty recording.
    pub recording: &'a EntityDb,

    /// Per-recording caches.
    pub caches: &'a StoreCache,

    /// The time control for the active recording.
    ///
    /// If none was created yet (or none is active), this points to a default time control.
    pub time_ctrl: &'a TimeControl,

    /// Should we enable the heuristics during this frame?
    pub should_enable_heuristics: bool,
}

impl ActiveStoreContext<'_> {
    pub fn is_active(&self, store_id: &StoreId) -> bool {
        self.recording.store_id() == store_id || self.blueprint.store_id() == store_id
    }

    pub fn application_id(&self) -> &ApplicationId {
        re_log::debug_assert!(
            self.recording.application_id() != StoreHub::welcome_screen_app_id(),
            "Bug: we should not be treating the welcome screen as a recording"
        );
        self.recording.application_id()
    }

    pub fn recording_store_id(&self) -> &StoreId {
        re_log::debug_assert!(self.recording.store_id() != &StoreId::empty_recording());
        self.recording.store_id()
    }

    /// The active recording
    pub fn recording(&self) -> &EntityDb {
        re_log::debug_assert!(self.recording.store_id() != &StoreId::empty_recording());
        self.recording
    }

    /// Currently selected section of time, if any.
    pub fn loop_selection(
        &self,
    ) -> Option<(re_log_types::TimelineName, re_log_types::AbsoluteTimeRangeF)> {
        self.time_ctrl
            .time_selection()
            .map(|q| (*self.time_ctrl.timeline_name(), q))
    }

    /// Accesses a memoization cache for reading and writing.
    ///
    /// Shorthand for `self.caches.memoizer(f)`.
    pub fn memoizer<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        self.caches.memoizer(f)
    }

    /// Accesses an existing memoization cache for reading.
    ///
    /// Shorthand for `self.caches.memoizer_read(f)`.
    pub fn memoizer_read<C: Cache, R>(&self, f: impl FnOnce(&C) -> R) -> Option<R> {
        self.caches.memoizer_read(f)
    }

    /// Tries to read an existing memoization cache entry, then computes it through mutable access on miss.
    ///
    /// Use this if you're working with init-only cache entries, expect your cache entry to be usually present
    /// and want to avoid the overhead of a write lock.
    /// Note that this _adds_ overhead for the miss path compared to `memoizer`, so don't use this if you expect many misses!
    /// (UI code typically doesn't need to care about this optimization, since it's usually single-threaded already.)
    ///
    /// Shorthand for `self.caches.memoizer_read_or_compute(key)`.
    pub fn memoizer_read_or_compute<C, Key, Value>(&self, key: &Key) -> Value
    where
        C: CacheEntryAccess<Key, Value> + Default,
    {
        self.caches.memoizer_read_or_compute::<C, Key, Value>(key)
    }
}
