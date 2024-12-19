use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber};
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
#[derive(Default, Clone)]
pub struct MaxImageDimensionsStoreSubscriber {
    max_dimensions: IntMap<EntityPath, MaxDimensions>,
}

impl MaxImageDimensionsStoreSubscriber {
    /// Accesses the image/video dimension information for a given store
    pub fn access<T>(
        store_id: &StoreId,
        f: impl FnOnce(&IntMap<EntityPath, MaxDimensions>) -> T,
    ) -> Option<T> {
        ChunkStore::with_per_store_subscriber_once(
            Self::subscription_handle(),
            store_id,
            move |subscriber: &Self| f(&subscriber.max_dimensions),
        )
    }
}

impl MaxImageDimensionsStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }
}

impl PerStoreChunkSubscriber for MaxImageDimensionsStoreSubscriber {
    #[inline]
    fn name() -> String {
        "MaxImageDimensionStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            if event.diff.kind != re_chunk_store::ChunkStoreDiffKind::Addition {
                // Max image dimensions are strictly additive
                continue;
            }

            // Handle `Image`, `DepthImage`, `SegmentationImage`…
            if let Some(all_dimensions) = event
                .diff
                .chunk
                .components()
                .get(&ImageFormat::name())
                .and_then(|per_desc| per_desc.values().next())
            {
                for new_dim in all_dimensions.iter().filter_map(|array| {
                    array.and_then(|array| {
                        ImageFormat::from_arrow2(&*array).ok()?.into_iter().next()
                    })
                }) {
                    let max_dim = self
                        .max_dimensions
                        .entry(event.diff.chunk.entity_path().clone())
                        .or_default();

                    max_dim.width = max_dim.width.max(new_dim.width);
                    max_dim.height = max_dim.height.max(new_dim.height);
                }
            }

            // Handle `ImageEncoded`, `AssetVideo`…
            let blobs = event.diff.chunk.iter_buffer(&Blob::name());
            let media_types = event.diff.chunk.iter_string(&MediaType::name());
            for (blob, media_type) in
                itertools::izip!(blobs, media_types.map(Some).chain(std::iter::repeat(None)))
            {
                if let Some(blob) = blob.first() {
                    if let Some([width, height]) = size_from_blob(
                        blob.as_slice(),
                        media_type.and_then(|v| v.first().map(|v| MediaType(v.clone().into()))),
                    ) {
                        let max_dim = self
                            .max_dimensions
                            .entry(event.diff.chunk.entity_path().clone())
                            .or_default();
                        max_dim.width = max_dim.width.max(width);
                        max_dim.height = max_dim.height.max(height);
                    }
                }
            }
        }
    }
}

fn size_from_blob(blob: &[u8], media_type: Option<MediaType>) -> Option<[u32; 2]> {
    re_tracing::profile_function!();

    let media_type = MediaType::or_guess_from_data(media_type, blob)?;

    if media_type.is_image() {
        re_tracing::profile_scope!("image");

        let image_bytes = blob;

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
        re_video::VideoData::load_from_bytes(blob, &media_type)
            .ok()
            .map(|video| video.dimensions())
    } else {
        None
    }
}
