use nohash_hasher::IntMap;
use once_cell::sync::OnceCell;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle, PerStoreChunkSubscriber};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{
    Archetype as _, ArchetypeName, Component as _, Loggable as _, archetypes, components,
    external::image,
};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ImageTypes: u8 {
        const IMAGE = 0b1;
        const ENCODED_IMAGE = 0b10;
        const SEGMENTATION_IMAGE = 0b100;
        const DEPTH_IMAGE = 0b1000;
        const VIDEO_ASSET = 0b10000;
        const VIDEO_STREAM = 0b100000;
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

    /// Keep track of all known video codecs.
    ///
    /// This makes it easy to access this minimal piece of information
    /// without doing a costly query which isn't possible inside of subscribers.
    /// Video codec per entity is expected to never change, doing so is regarded as user error.
    ///
    /// Note that this could be a separate subscriber, but it wasn't necessary so far.
    video_codecs: IntMap<EntityPathHash, components::VideoCodec>,
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

            let chunk = &event.diff.chunk;
            let components = chunk.components();
            let entity_path = chunk.entity_path();

            // Handle new video codecs first since we do a lookup on this later.
            if components.contains_key(&archetypes::VideoStream::descriptor_codec()) {
                for codec in chunk.iter_component::<components::VideoCodec>(
                    &archetypes::VideoStream::descriptor_codec(),
                ) {
                    let Some(codec) = codec.first() else {
                        continue;
                    }; // Ignore both empty arrays and multiple codecs per row.
                    if let Some(existing_codec) =
                        self.video_codecs.insert(entity_path.hash(), *codec)
                    {
                        if existing_codec != *codec {
                            re_log::warn!(
                                "Overwriting existing video codec for entity path {:?} from {:?} to {:?}. Video codec per entity is expected to never change.",
                                entity_path,
                                existing_codec,
                                codec,
                            );
                        }
                    }
                }
            }

            for (descr, list_array) in components.iter() {
                let Some(archetype_name) = descr.archetype_name else {
                    // Don't care about non-builtin types, therefore archetype name should be present.
                    continue;
                };

                // First try to detect the type of image.
                let Some(image_type) = [
                    (archetypes::Image::name(), ImageTypes::IMAGE),
                    (
                        archetypes::SegmentationImage::name(),
                        ImageTypes::SEGMENTATION_IMAGE,
                    ),
                    (archetypes::EncodedImage::name(), ImageTypes::ENCODED_IMAGE),
                    (archetypes::DepthImage::name(), ImageTypes::DEPTH_IMAGE),
                    (archetypes::AssetVideo::name(), ImageTypes::VIDEO_ASSET),
                    (archetypes::VideoStream::name(), ImageTypes::VIDEO_STREAM),
                ]
                .iter()
                .find_map(|(image_archetype_name, image_type)| {
                    (&archetype_name == image_archetype_name).then_some(*image_type)
                }) else {
                    // Early out if there's no image type detected.
                    continue;
                };

                let max_dim = self.max_dimensions.entry(entity_path.clone()).or_default();
                max_dim.image_types.insert(image_type);

                // Size detection for various types of components.
                if descr.component_name == Some(components::ImageFormat::name()) {
                    for new_dim in list_array.iter().filter_map(|array| {
                        array.and_then(|array| {
                            let array = arrow::array::ArrayRef::from(array);
                            components::ImageFormat::from_arrow(&array)
                                .ok()?
                                .into_iter()
                                .next()
                        })
                    }) {
                        max_dim.width = max_dim.width.max(new_dim.width);
                        max_dim.height = max_dim.height.max(new_dim.height);
                    }
                } else if descr.component_name == Some(components::Blob::name()) {
                    let blobs = chunk.iter_slices::<&[u8]>(descr.clone());

                    // Is there a media type paired up with this blob?
                    let media_type_descr = components.keys().find(|desc| {
                        desc.component_name == Some(components::MediaType::name())
                            && desc.archetype_name == descr.archetype_name
                    });
                    let media_types = media_type_descr.map_or(Vec::new(), |media_type_descr| {
                        chunk
                            .iter_slices::<String>(media_type_descr.clone())
                            .collect()
                    });
                    for (blob, media_type) in itertools::izip!(
                        blobs,
                        media_types
                            .into_iter()
                            .map(Some)
                            .chain(std::iter::repeat(None))
                    ) {
                        let Some(blob) = blob.first() else {
                            continue;
                        };

                        let media_type = media_type.and_then(|v| {
                            v.first().map(|v| components::MediaType(v.clone().into()))
                        });
                        if let Some([width, height]) = try_size_from_blob(
                            blob,
                            media_type,
                            archetype_name,
                            &entity_path.to_string(),
                        ) {
                            max_dim.width = max_dim.width.max(width);
                            max_dim.height = max_dim.height.max(height);
                        }
                    }
                } else if descr.component_name == Some(components::VideoSample::name()) {
                    let Some(video_codec) = self.video_codecs.get(&entity_path.hash()).copied()
                    else {
                        // Codec is typically logged earlier.
                        continue;
                    };

                    for sample in chunk.iter_slices::<&[u8]>(descr.clone()) {
                        let Some(sample) = sample.first() else {
                            continue;
                        };
                        if let Some([width, height]) =
                            try_size_from_video_stream_sample(sample, video_codec)
                        {
                            max_dim.width = max_dim.width.max(width);
                            max_dim.height = max_dim.height.max(height);
                        }
                    }
                }
            }
        }
    }
}

fn try_size_from_blob(
    blob: &[u8],
    media_type: Option<components::MediaType>,
    archetype_name: ArchetypeName,
    debug_name: &str,
) -> Option<[u32; 2]> {
    re_tracing::profile_function!();

    if archetype_name == archetypes::EncodedImage::name() {
        re_tracing::profile_scope!("image");

        let media_type = components::MediaType::or_guess_from_data(media_type, blob);
        let mut reader = image::ImageReader::new(std::io::Cursor::new(blob));

        if let Some(format) = media_type.and_then(|mt| image::ImageFormat::from_mime_type(&mt.0)) {
            reader.set_format(format);
        } else if let Ok(format) = image::guess_format(blob) {
            // Weirdly enough, `reader.decode` doesn't do this for us.
            reader.set_format(format);
        }

        reader.into_dimensions().ok().map(|size| size.into())
    } else if archetype_name == archetypes::AssetVideo::name() {
        re_tracing::profile_scope!("video asset");

        let media_type = components::MediaType::or_guess_from_data(media_type, blob)?;
        re_video::VideoDataDescription::load_from_bytes(blob, media_type.as_str(), debug_name)
            .ok()
            .and_then(|video| video.encoding_details.map(|e| e.coded_dimensions))
            .map(|[w, h]| [w as _, h as _])
    } else {
        None
    }
}

fn try_size_from_video_stream_sample(
    sample: &[u8],
    video_codec: components::VideoCodec,
) -> Option<[u32; 2]> {
    let codec = match video_codec {
        components::VideoCodec::H264 => re_video::VideoCodec::H264,
    };

    match re_video::detect_gop_start(sample, codec).ok()? {
        re_video::GopStartDetection::StartOfGop(descr) => Some([
            descr.coded_dimensions[0] as _,
            descr.coded_dimensions[1] as _,
        ]),
        re_video::GopStartDetection::NotStartOfGop => None,
    }
}
