use std::sync::OnceLock;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber};
use re_log_types::StoreId;

use crate::transform_cache::TransformAspect;

/// Store subscriber that works hand in hand with [`TransformCache`] to track all needed changes to the [`TransformCache`]
/// as new data comes in.
#[derive(Default)]
pub struct TransformCacheStoreSubscriber {
    unprocessed_events: Vec<(re_chunk_store::ChunkStoreEvent, TransformAspect)>,
}

impl TransformCacheStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Ensures the subscriber is registered.
    pub fn ensure_registered() {
        let _ = Self::subscription_handle();
    }

    /// Retrieves all transform events that have not been processed yet since the last call to this function.
    pub fn take_transform_events(
        store_id: &StoreId,
    ) -> Vec<(re_chunk_store::ChunkStoreEvent, TransformAspect)> {
        ChunkStore::with_per_store_subscriber_mut(
            Self::subscription_handle(),
            store_id,
            |subscriber: &mut Self| std::mem::take(&mut subscriber.unprocessed_events),
        )
        .unwrap_or_default()
    }
}

impl PerStoreChunkSubscriber for TransformCacheStoreSubscriber {
    fn name() -> String {
        "rerun.TransformCache".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            // The components we are interested in may only show up on some of the timelines
            // within this chunk, so strictly speaking the affected "aspects" we compute here are conservative.
            // But that's fairly rare, so a few false positive entries here are fine.
            let mut aspects = TransformAspect::empty();
            for component_type in event
                .chunk
                .component_descriptors()
                .filter_map(|c| c.component_type)
            {
                aspects |= TransformAspect::from_component_type(component_type);
            }
            if aspects.is_empty() {
                continue;
            }

            self.unprocessed_events.push((event.clone(), aspects));
        }
    }
}
