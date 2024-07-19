use ahash::HashMap;
use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriber, ChunkStoreSubscriberHandle};
use re_log_types::{EntityPath, StoreId};
use re_types::{components::Resolution2D, Loggable};

#[derive(Default, Debug, Clone)]
pub struct MaxImageDimensions(IntMap<EntityPath, Resolution2D>);

impl MaxImageDimensions {
    /// Accesses the image dimension information for a given store
    pub fn access<T>(
        store_id: &StoreId,
        f: impl FnOnce(&IntMap<EntityPath, Resolution2D>) -> T,
    ) -> Option<T> {
        ChunkStore::with_subscriber_once(
            MaxImageDimensionSubscriber::subscription_handle(),
            move |subscriber: &MaxImageDimensionSubscriber| {
                subscriber.max_dimensions.get(store_id).map(|v| &v.0).map(f)
            },
        )
        .flatten()
    }
}

#[derive(Default)]
pub struct MaxImageDimensionSubscriber {
    max_dimensions: HashMap<StoreId, MaxImageDimensions>,
}

impl MaxImageDimensionSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| ChunkStore::register_subscriber(Box::<Self>::default()))
    }
}

impl ChunkStoreSubscriber for MaxImageDimensionSubscriber {
    #[inline]
    fn name(&self) -> String {
        "MaxImageDimensionStoreSubscriber".to_owned()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[re_chunk_store::ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            if event.diff.kind != re_chunk_store::ChunkStoreDiffKind::Addition {
                // Max image dimensions are strictly additive
                continue;
            }

            if let Some(all_dimensions) = event.diff.chunk.components().get(&Resolution2D::name()) {
                for new_dim in all_dimensions.iter().filter_map(|array| {
                    array.and_then(|array| {
                        Resolution2D::from_arrow(&*array).ok()?.into_iter().next()
                    })
                }) {
                    let max_dim = self
                        .max_dimensions
                        .entry(event.store_id.clone())
                        .or_default()
                        .0
                        .entry(event.diff.chunk.entity_path().clone())
                        .or_default();

                    max_dim.set_height(max_dim.height().max(new_dim.height()));
                    max_dim.set_width(max_dim.width().max(new_dim.width()));
                }
            }
        }
    }
}
