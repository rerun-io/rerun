use parking_lot::RwLock;

use crate::{DataStore, StoreEvent};

// ---

// TODO(cmc): Not sure why I need the extra Box here, RwLock should be `?Sized`.
type SharedStoreView = RwLock<Box<dyn StoreView>>;

/// A [`StoreView`] subscribes to atomic changes in one or more [`DataStore`]s through [`StoreEvent`]s.
///
/// [`StoreView`]s can be used to build both secondary indices and trigger systems.
///
/// Check out our [`custom_store_view`] example to see it in action.
///
/// [`custom_store_view`]: https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_store_view?speculative-link
//
// TODO(#4204): StoreView should require SizeBytes so they can be part of memstats.
pub trait StoreView: std::any::Any + Send + Sync {
    /// Arbitrary name for the view.
    ///
    /// Does not need to be unique.
    fn name(&self) -> String;

    /// Workaround for downcasting support, simply return `self`:
    /// ```ignore
    /// fn as_any(&self) -> &dyn std::any::Any {
    ///     self
    /// }
    /// ```
    fn as_any(&self) -> &dyn std::any::Any;

    /// Workaround for downcasting support, simply return `self`:
    /// ```ignore
    /// fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
    ///     self
    /// }
    /// ```
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// The core of this trait: get notified of changes happening in one or more [`DataStore`]s.
    ///
    /// This will be called automatically by the [`DataStore`] itself if the view has been
    /// registered: [`DataStore::register_view`].
    /// Or you might want to feed it [`StoreEvent`]s manually, depending on your use case.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// fn on_events(&mut self, events: &[StoreEvent]) {
    ///     use re_arrow_store::StoreDiffKind;
    ///     for event in events {
    ///         match event.kind {
    ///             StoreDiffKind::Addition => println!("Row added: {}", event.row_id),
    ///             StoreDiffKind::Deletion => println!("Row removed: {}", event.row_id),
    ///         }
    ///     }
    /// }
    /// ```
    fn on_events(&mut self, events: &[StoreEvent]);
}

/// All registered [`StoreView`]s.
static VIEWS: once_cell::sync::Lazy<RwLock<Vec<SharedStoreView>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

#[derive(Debug, Clone, Copy)]
pub struct StoreViewHandle(u32);

impl DataStore {
    /// Registers a [`StoreView`] so it gets automatically notified when data gets added and/or
    /// removed to/from a [`DataStore`].
    ///
    /// Refer to [`StoreEvent`]'s documentation for more information about these events.
    ///
    /// ## Scope
    ///
    /// Registered [`StoreView`]s are global scope: they get notified of all events from all
    /// existing [`DataStore`]s, including [`DataStore`]s created after the view was registered.
    ///
    /// Use [`StoreEvent::store_id`] to identify the source of an event.
    ///
    /// ## Late registration
    ///
    /// Views must be registered before a store gets created to guarantee that no events were
    /// missed.
    ///
    /// [`StoreEvent::event_id`] can be used to identify missing events.
    ///
    /// ## Ordering
    ///
    /// The order in which registered views are notified is undefined and will likely become
    /// concurrent in the future.
    ///
    /// If you need a specific order across multiple views, embed them into an orchestrating view.
    //
    // TODO(cmc): send a compacted snapshot to late registerers for bootstrapping
    pub fn register_view(view: Box<dyn StoreView>) -> StoreViewHandle {
        let mut views = VIEWS.write();
        views.push(RwLock::new(view));
        StoreViewHandle(views.len() as u32 - 1)
    }

    /// Passes a reference to the downcasted view to the given callback.
    ///
    /// Returns `None` if the view doesn't exist or downcasting failed.
    pub fn with_view<V: StoreView, T, F: FnMut(&V) -> T>(
        StoreViewHandle(handle): StoreViewHandle,
        mut f: F,
    ) -> Option<T> {
        let views = VIEWS.read();
        views.get(handle as usize).and_then(|view| {
            let view = view.read();
            view.as_any().downcast_ref::<V>().map(&mut f)
        })
    }

    /// Passes a mutable reference to the downcasted view to the given callback.
    ///
    /// Returns `None` if the view doesn't exist or downcasting failed.
    pub fn with_view_mut<V: StoreView, T, F: FnMut(&mut V) -> T>(
        StoreViewHandle(handle): StoreViewHandle,
        mut f: F,
    ) -> Option<T> {
        let views = VIEWS.read();
        views.get(handle as usize).and_then(|view| {
            let mut view = view.write();
            view.as_any_mut().downcast_mut::<V>().map(&mut f)
        })
    }

    /// Called by [`DataStore`]'s mutating methods to notify view subscribers of upcoming events.
    pub(crate) fn on_events(events: &[StoreEvent]) {
        let views = VIEWS.read();
        // TODO(cmc): might want to parallelize at some point.
        for view in views.iter() {
            view.write().on_events(events);
        }
    }
}
