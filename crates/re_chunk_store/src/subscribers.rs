use parking_lot::RwLock;

use crate::{ChunkStore, ChunkStoreEvent};

// ---

// TODO(cmc): Not sure why I need the extra Box here, RwLock should be `?Sized`.
type SharedStoreSubscriber = RwLock<Box<dyn ChunkStoreSubscriber>>;

/// A [`ChunkStoreSubscriber`] subscribes to atomic changes from all [`ChunkStore`]s
/// through [`ChunkStoreEvent`]s.
///
/// [`ChunkStoreSubscriber`]s can be used to build both secondary indices and trigger systems.
//
// TODO(#4204): StoreSubscriber should require SizeBytes so they can be part of memstats.
pub trait ChunkStoreSubscriber: std::any::Any + Send + Sync {
    /// Arbitrary name for the subscriber.
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

    /// The core of this trait: get notified of changes happening in all [`ChunkStore`]s.
    ///
    /// This will be called automatically by the [`ChunkStore`] itself if the subscriber has been
    /// registered: [`ChunkStore::register_subscriber`].
    /// Or you might want to feed it [`ChunkStoreEvent`]s manually, depending on your use case.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// fn on_events(&mut self, events: &[ChunkStoreEvent]) {
    ///     use re_chunk_store::ChunkStoreDiffKind;
    ///     for event in events {
    ///         match event.kind {
    ///             ChunkStoreDiffKind::Addition => println!("Row added: {}", event.row_id),
    ///             ChunkStoreDiffKind::Deletion => println!("Row removed: {}", event.row_id),
    ///         }
    ///     }
    /// }
    /// ```
    fn on_events(&mut self, events: &[ChunkStoreEvent]);
}

/// All registered [`ChunkStoreSubscriber`]s.
static SUBSCRIBERS: once_cell::sync::Lazy<RwLock<Vec<SharedStoreSubscriber>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

#[derive(Debug, Clone, Copy)]
pub struct ChunkStoreSubscriberHandle(u32);

impl ChunkStore {
    /// Registers a [`ChunkStoreSubscriber`] so it gets automatically notified when data gets added and/or
    /// removed to/from a [`ChunkStore`].
    ///
    /// Refer to [`ChunkStoreEvent`]'s documentation for more information about these events.
    ///
    /// ## Scope
    ///
    /// Registered [`ChunkStoreSubscriber`]s are global scope: they get notified of all events from all
    /// existing [`ChunkStore`]s, including [`ChunkStore`]s created after the subscriber was registered.
    ///
    /// Use [`ChunkStoreEvent::store_id`] to identify the source of an event.
    ///
    /// ## Late registration
    ///
    /// Subscribers must be registered before a store gets created to guarantee that no events
    /// were missed.
    ///
    /// [`ChunkStoreEvent::event_id`] can be used to identify missing events.
    ///
    /// ## Ordering
    ///
    /// The order in which registered subscribers are notified is undefined and will likely become
    /// concurrent in the future.
    ///
    /// If you need a specific order across multiple subscribers, embed them into an orchestrating
    /// subscriber.
    //
    // TODO(cmc): send a compacted snapshot to late registerers for bootstrapping
    pub fn register_subscriber(
        subscriber: Box<dyn ChunkStoreSubscriber>,
    ) -> ChunkStoreSubscriberHandle {
        let mut subscribers = SUBSCRIBERS.write();
        subscribers.push(RwLock::new(subscriber));
        ChunkStoreSubscriberHandle(subscribers.len() as u32 - 1)
    }

    /// Passes a reference to the downcasted subscriber to the given `FnMut` callback.
    ///
    /// Returns `None` if the subscriber doesn't exist or downcasting failed.
    pub fn with_subscriber<V: ChunkStoreSubscriber, T, F: FnMut(&V) -> T>(
        ChunkStoreSubscriberHandle(handle): ChunkStoreSubscriberHandle,
        mut f: F,
    ) -> Option<T> {
        let subscribers = SUBSCRIBERS.read();
        subscribers.get(handle as usize).and_then(|subscriber| {
            let subscriber = subscriber.read();
            subscriber.as_any().downcast_ref::<V>().map(&mut f)
        })
    }

    /// Passes a reference to the downcasted subscriber to the given `FnOnce` callback.
    ///
    /// Returns `None` if the subscriber doesn't exist or downcasting failed.
    pub fn with_subscriber_once<V: ChunkStoreSubscriber, T, F: FnOnce(&V) -> T>(
        ChunkStoreSubscriberHandle(handle): ChunkStoreSubscriberHandle,
        f: F,
    ) -> Option<T> {
        let subscribers = SUBSCRIBERS.read();
        subscribers.get(handle as usize).and_then(|subscriber| {
            let subscriber = subscriber.read();
            subscriber.as_any().downcast_ref::<V>().map(f)
        })
    }

    /// Passes a mutable reference to the downcasted subscriber to the given callback.
    ///
    /// Returns `None` if the subscriber doesn't exist or downcasting failed.
    pub fn with_subscriber_mut<V: ChunkStoreSubscriber, T, F: FnMut(&mut V) -> T>(
        ChunkStoreSubscriberHandle(handle): ChunkStoreSubscriberHandle,
        mut f: F,
    ) -> Option<T> {
        let subscribers = SUBSCRIBERS.read();
        subscribers.get(handle as usize).and_then(|subscriber| {
            let mut subscriber = subscriber.write();
            subscriber.as_any_mut().downcast_mut::<V>().map(&mut f)
        })
    }

    /// Called by [`ChunkStore`]'s mutating methods to notify subscriber subscribers of upcoming events.
    pub(crate) fn on_events(events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();
        let subscribers = SUBSCRIBERS.read();
        // TODO(cmc): might want to parallelize at some point.
        for subscriber in subscribers.iter() {
            subscriber.write().on_events(events);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ahash::HashSet;

    use re_chunk::{Chunk, RowId};
    use re_log_types::{
        example_components::{MyColor, MyIndex, MyPoint},
        StoreId, TimePoint, Timeline,
    };

    use crate::{ChunkStore, ChunkStoreSubscriber, GarbageCollectionOptions};

    use super::*;

    /// A simple [`ChunkStoreSubscriber`] for test purposes that just accumulates [`ChunkStoreEvent`]s.
    #[derive(Debug)]
    struct AllEvents {
        store_ids: HashSet<StoreId>,
        events: Vec<ChunkStoreEvent>,
    }

    impl AllEvents {
        fn new(store_ids: impl IntoIterator<Item = StoreId>) -> Self {
            Self {
                store_ids: store_ids.into_iter().collect(),
                events: Vec::new(),
            }
        }
    }

    impl ChunkStoreSubscriber for AllEvents {
        fn name(&self) -> String {
            "rerun.testing.store_subscribers.AllEvents".into()
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn on_events(&mut self, events: &[ChunkStoreEvent]) {
            self.events.extend(
                events
                    .iter()
                    // NOTE: `cargo` implicitly runs tests in parallel!
                    .filter(|e| self.store_ids.contains(&e.store_id))
                    .cloned(),
            );
        }
    }

    #[test]
    fn store_subscriber() -> anyhow::Result<()> {
        let mut store1 = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let mut expected_events = Vec::new();

        let view = AllEvents::new([store1.id().clone(), store.id().clone()]);
        let view_handle = ChunkStore::register_subscriber(Box::new(view));

        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_temporal("other");
        let timeline_yet_another = Timeline::new_sequence("yet_another");

        let chunk = Chunk::builder("entity_a".into())
            .with_component_batch(
                RowId::new(),
                TimePoint::from_iter([
                    (timeline_frame, 42),      //
                    (timeline_other, 666),     //
                    (timeline_yet_another, 1), //
                ]),
                &MyIndex::from_iter(0..10),
            )
            .build()?;

        expected_events.extend(store1.insert_chunk(&Arc::new(chunk))?);

        let chunk = {
            let num_instances = 3;
            let points: Vec<_> = (0..num_instances)
                .map(|i| MyPoint::new(0.0, i as f32))
                .collect();
            let colors = vec![MyColor::from(0xFF0000FF)];
            Chunk::builder("entity_b".into())
                .with_component_batches(
                    RowId::new(),
                    TimePoint::from_iter([
                        (timeline_frame, 42),      //
                        (timeline_yet_another, 1), //
                    ]),
                    [&points as _, &colors as _],
                )
                .build()?
        };

        expected_events.extend(store.insert_chunk(&Arc::new(chunk))?);

        let chunk = {
            let num_instances = 6;
            let colors = vec![MyColor::from(0x00DD00FF); num_instances];
            Chunk::builder("entity_b".into())
                .with_component_batches(
                    RowId::new(),
                    TimePoint::default(),
                    [
                        &MyIndex::from_iter(0..num_instances as _) as _,
                        &colors as _,
                    ],
                )
                .build()?
        };

        expected_events.extend(store1.insert_chunk(&Arc::new(chunk))?);

        expected_events.extend(store1.gc(&GarbageCollectionOptions::gc_everything()).0);
        expected_events.extend(store.gc(&GarbageCollectionOptions::gc_everything()).0);

        ChunkStore::with_subscriber::<AllEvents, _, _>(view_handle, |got| {
            similar_asserts::assert_eq!(expected_events.len(), got.events.len());
            similar_asserts::assert_eq!(expected_events, got.events);
        });

        Ok(())
    }
}
