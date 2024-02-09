use ahash::HashMap;
use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;
use re_data_store::{StoreSubscriber, StoreSubscriberHandle};
use re_log_types::{EntityPath, StoreId};
use re_types::{components::TensorData, Loggable};

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageDimensions {
    pub height: u64,
    pub width: u64,
    pub channels: u64,
}

#[derive(Default, Debug, Clone)]
pub struct MaxImageDimensions(IntMap<EntityPath, ImageDimensions>);

impl MaxImageDimensions {
    /// Accesses the image dimension information for a given store
    pub fn access<T>(
        store_id: &StoreId,
        f: impl FnOnce(&IntMap<EntityPath, ImageDimensions>) -> T,
    ) -> Option<T> {
        re_data_store::DataStore::with_subscriber_once(
            MaxImageDimensionSubscriber::subscription_handle(),
            move |subscriber: &MaxImageDimensionSubscriber| {
                subscriber.max_dimensions.get(store_id).map(|v| &v.0).map(f)
            },
        )
        .flatten()
    }
}

#[derive(Default)]
struct MaxImageDimensionSubscriber {
    max_dimensions: HashMap<StoreId, MaxImageDimensions>,
}

impl MaxImageDimensionSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> StoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<re_data_store::StoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| {
            re_data_store::DataStore::register_subscriber(
                Box::<MaxImageDimensionSubscriber>::default(),
            )
        })
    }
}

impl StoreSubscriber for MaxImageDimensionSubscriber {
    fn name(&self) -> String {
        "MaxImageDimensionStoreSubscriber".to_owned()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[re_data_store::StoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            if event.diff.kind != re_data_store::StoreDiffKind::Addition {
                // Max image dimensions are strictly additive
                continue;
            }

            if let Some(cell) = event.diff.cells.get(&TensorData::name()) {
                if let Ok(Some(tensor_data)) = cell.try_to_native_mono::<TensorData>() {
                    if let Some([height, width, channels]) =
                        tensor_data.image_height_width_channels()
                    {
                        let dimensions = self
                            .max_dimensions
                            .entry(event.store_id.clone())
                            .or_default()
                            .0
                            .entry(event.diff.entity_path.clone())
                            .or_default();

                        dimensions.height = dimensions.height.max(height);
                        dimensions.width = dimensions.width.max(width);
                        dimensions.channels = dimensions.channels.max(channels);
                    }
                }
            }
        }
    }
}
