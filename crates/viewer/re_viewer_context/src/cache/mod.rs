//! Viewer caches
//!
//! Caches are registered lazily upon first use, see [`Caches::entry`].
//! The concrete caches exposed here are always available for all viewer crates.

mod caches;
mod image_decode_cache;
mod image_stats_cache;
mod tensor_stats_cache;
mod transform_database_store;
mod video_asset_cache;
mod video_stream_cache;

pub use caches::{Cache, Caches};
// TODO(andreas): Do we _really_ have to have all these caches in `re_viewer_context`?
// Caches are fully dynamic and registration based, so they can be added at runtime by any crate.
// The reason this happens it that various viewer crates wants to access these, mostly for ui purposes.
// Ideally, they would only depend on the ones needed.
pub use image_decode_cache::ImageDecodeCache;
pub use image_stats_cache::ImageStatsCache;
pub use tensor_stats_cache::TensorStatsCache;
pub use transform_database_store::TransformDatabaseStoreCache;
pub use video_asset_cache::VideoAssetCache;
pub use video_stream_cache::{
    SharablePlayableVideoStream, VideoStreamCache, VideoStreamProcessingError,
};

// ----

fn filter_blob_removed_events(
    events: &[&re_chunk_store::ChunkStoreEvent],
) -> ahash::HashSet<crate::StoredBlobCacheKey> {
    use re_sdk_types::Component as _;

    events
        .iter()
        .flat_map(|event| {
            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                itertools::Either::Left(
                    event
                        .chunk_before_processing
                        .component_descriptors()
                        .filter(|descr| {
                            descr.component_type == Some(re_sdk_types::components::Blob::name())
                        })
                        .flat_map(|descr| {
                            event.chunk_before_processing.row_ids().map(move |row_id| {
                                crate::StoredBlobCacheKey::new(row_id, descr.component)
                            })
                        }),
                )
            } else {
                itertools::Either::Right(std::iter::empty())
            }
        })
        .collect()
}
