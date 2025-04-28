use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber};
use re_log_types::{EntityPath, StoreId};
use re_types::{
    archetypes,
    components::{Blob, ImageFormat, MediaType},
    external::image,
    Component as _, Loggable as _,
};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ImageTypes: u8 {
        const IMAGE = 0b0001;
        const ENCODED_IMAGE = 0b0010;
        const SEGMENTATION_IMAGE = 0b0100;
        const DEPTH_IMAGE = 0b1000;
        const VIDEO = 0b10000;
    }
}

#[derive(Debug, Clone, Default)]
pub struct MaxDimensions {
    pub width: u32,
    pub height: u32,
    pub image_types: ImageTypes,
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
            let components = event.diff.chunk.components();
            for image_format_list_array in components.get_by_component_name(ImageFormat::name()) {
                for new_dim in image_format_list_array.iter().filter_map(|array| {
                    array.and_then(|array| {
                        let array = arrow::array::ArrayRef::from(array);
                        ImageFormat::from_arrow(&array).ok()?.into_iter().next()
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
            // TODO(andreas): this should be part of the ImageFormat component's tag instead.
            if components.contains_key(&archetypes::Image::descriptor_indicator()) {
                self.max_dimensions
                    .entry(event.diff.chunk.entity_path().clone())
                    .or_default()
                    .image_types
                    .insert(ImageTypes::IMAGE);
            }
            if components.contains_key(&archetypes::SegmentationImage::descriptor_indicator()) {
                self.max_dimensions
                    .entry(event.diff.chunk.entity_path().clone())
                    .or_default()
                    .image_types
                    .insert(ImageTypes::SEGMENTATION_IMAGE);
            }
            if components.contains_key(&archetypes::DepthImage::descriptor_indicator()) {
                self.max_dimensions
                    .entry(event.diff.chunk.entity_path().clone())
                    .or_default()
                    .image_types
                    .insert(ImageTypes::DEPTH_IMAGE);
            }

            // Handle `ImageEncoded`, `AssetVideo`…
            let blobs = event.diff.chunk.iter_slices::<&[u8]>(Blob::name());
            let media_types = event.diff.chunk.iter_slices::<String>(MediaType::name());
            for (blob, media_type) in
                itertools::izip!(blobs, media_types.map(Some).chain(std::iter::repeat(None)))
            {
                if let Some(blob) = blob.first() {
                    let media_type =
                        media_type.and_then(|v| v.first().map(|v| MediaType(v.clone().into())));
                    let Some(media_type) = MediaType::or_guess_from_data(media_type, blob) else {
                        continue;
                    };

                    if let Some([width, height]) = size_from_blob(blob, &media_type) {
                        let max_dim = self
                            .max_dimensions
                            .entry(event.diff.chunk.entity_path().clone())
                            .or_default();
                        max_dim.width = max_dim.width.max(width);
                        max_dim.height = max_dim.height.max(height);

                        // TODO(andreas): this should be part of the Blob component's tag instead.
                        if media_type.is_image() {
                            max_dim.image_types.insert(ImageTypes::ENCODED_IMAGE);
                        } else if media_type.is_video() {
                            max_dim.image_types.insert(ImageTypes::VIDEO);
                        }
                    }
                }
            }
        }
    }
}

fn size_from_blob(blob: &[u8], media_type: &MediaType) -> Option<[u32; 2]> {
    re_tracing::profile_function!();

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
        re_video::VideoData::load_from_bytes(blob, media_type)
            .ok()
            .map(|video| video.dimensions())
    } else {
        None
    }
}
