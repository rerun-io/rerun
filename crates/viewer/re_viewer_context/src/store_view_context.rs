use re_chunk::{Timeline, TimelineName};
use re_entity_db::EntityDb;

use crate::{AppContext, AppOptions, Cache, CacheEntryAccess, StoreCache, TimeControl};

/// Context for viewing a specific store,
/// (either a recording, or a blueprint).
///
/// Never use [`StoreViewContext`] where [`AppContext`] would suffice.
#[derive(Clone)]
pub struct StoreViewContext<'a> {
    pub app_ctx: &'a AppContext<'a>,

    /// The store we are viewing
    pub db: &'a EntityDb,

    /// Where the time cursor is at etc
    pub time_ctrl: &'a TimeControl,

    /// Needed to display images, videos, etc
    pub caches: &'a StoreCache,
}

impl<'a> std::ops::Deref for StoreViewContext<'a> {
    type Target = AppContext<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.app_ctx
    }
}

impl<'a> StoreViewContext<'a> {
    /// Move time cursor
    #[must_use]
    pub fn with_time_ctrl(&self, time_ctrl: &'a TimeControl) -> Self {
        Self {
            time_ctrl,
            ..self.clone()
        }
    }

    /// The current time cursor
    pub fn query(&self) -> re_chunk_store::LatestAtQuery {
        self.time_ctrl.current_query()
    }

    /// The currently selected timeline for this store.
    pub fn timeline_name(&self) -> TimelineName {
        self.query().timeline()
    }

    /// The currently selected timeline for this store.
    pub fn timeline(&self) -> Timeline {
        let name = self.timeline_name();
        let typ = self.db.timeline_type(&name);
        Timeline::new(name, typ)
    }

    pub fn render_ctx(&self) -> &re_renderer::RenderContext {
        self.app_ctx.render_ctx
    }

    pub fn app_options(&self) -> &AppOptions {
        self.app_ctx.app_options
    }

    pub fn component_ui_registry(&self) -> &crate::ComponentUiRegistry {
        self.app_ctx.component_ui_registry
    }

    pub fn command_sender(&self) -> &crate::CommandSender {
        self.app_ctx.command_sender
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
