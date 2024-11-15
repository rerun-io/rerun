use ahash::HashMap;
use arrow2::array::Array;
use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriber, ChunkStoreSubscriberHandle};
use re_log_types::{EntityPath, StoreId};
use re_types::{
    components::{Blob, ImageFormat, MediaType},
    external::image,
    Component, Loggable,
};

#[derive(Debug, Clone, Default)]
pub struct MaxDimensions {
    pub width: u32,
    pub height: u32,
}

/// The size of the largest image and/or video at a given entity path.
#[derive(Default, Debug, Clone)]
pub struct MaxImageDimensions(IntMap<EntityPath, MaxDimensions>);

impl MaxImageDimensions {
    /// Accesses the image/video dimension information for a given store
    pub fn access<T>(
        store_id: &StoreId,
        f: impl FnOnce(&IntMap<EntityPath, MaxDimensions>) -> T,
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

            // Handle `Image`, `DepthImage`, `SegmentationImage`…
            if let Some(all_dimensions) = event.diff.chunk.components().get(&ImageFormat::name()) {
                for new_dim in all_dimensions.iter().filter_map(|array| {
                    array
                        .and_then(|array| ImageFormat::from_arrow(&*array).ok()?.into_iter().next())
                }) {
                    let max_dim = self
                        .max_dimensions
                        .entry(event.store_id.clone())
                        .or_default()
                        .0
                        .entry(event.diff.chunk.entity_path().clone())
                        .or_default();

                    max_dim.width = max_dim.width.max(new_dim.width);
                    max_dim.height = max_dim.height.max(new_dim.height);
                }
            }

            // Handle `ImageEncoded`, `AssetVideo`…
            let blobs = event.diff.chunk.iter_component_arrays(&Blob::name());
            let media_types = event.diff.chunk.iter_component_arrays(&MediaType::name());
            for (blob, media_type) in
                itertools::izip!(blobs, media_types.map(Some).chain(std::iter::repeat(None)))
            {
                if let Some([width, height]) = size_from_blob(blob.as_ref(), media_type.as_deref())
                {
                    let max_dim = self
                        .max_dimensions
                        .entry(event.store_id.clone())
                        .or_default()
                        .0
                        .entry(event.diff.chunk.entity_path().clone())
                        .or_default();
                    max_dim.width = max_dim.width.max(width);
                    max_dim.height = max_dim.height.max(height);
                }
            }
        }
    }
}

fn size_from_blob(blob: &dyn Array, media_type: Option<&dyn Array>) -> Option<[u32; 2]> {
    re_tracing::profile_function!();

    let blob = Blob::from_arrow_opt(blob).ok()?.first()?.clone()?;

    let media_type: Option<MediaType> = media_type
        .and_then(|media_type| MediaType::from_arrow_opt(media_type).ok())
        .and_then(|list| list.first().cloned())
        .flatten();

    let media_type = MediaType::or_guess_from_data(media_type, &blob)?;

    if media_type.is_image() {
        re_tracing::profile_scope!("image");

        let image_bytes = blob.0.as_slice();

        let mut reader = image::ImageReader::new(std::io::Cursor::new(image_bytes));

        if let Some(format) = image::ImageFormat::from_mime_type(&media_type.0) {
            reader.set_format(format);
        }

        if reader.format().is_none() {
            if let Ok(format) = image::guess_format(image_bytes) {
                // Weirdly enough, `reader.decode` doesn't do this for us.
                reader.set_format(format);
            }
        }

        reader.into_dimensions().ok().map(|size| size.into())
    } else if media_type.is_video() {
        re_tracing::profile_scope!("video");
        re_video::VideoData::load_from_bytes(&blob, &media_type)
            .ok()
            .map(|video| video.dimensions())
    } else {
        None
    }
}
