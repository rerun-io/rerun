//! Core mp4-to-chunk loading logic for Rerun.
//!
//! Reads any `.mp4` file (or its in-memory bytes) and emits an iterator of
//! [`re_chunk::Chunk`]s ready to be sent to a recording or chunk store.
//!
//! The current single output mode is [`Mode::Asset`] — emit an
//! [`re_sdk_types::archetypes::AssetVideo`] blob chunk plus a
//! [`re_sdk_types::archetypes::VideoFrameReference`] index chunk. This is the
//! path used by the file importer (`rerun video.mp4`).
//!
//! The entry point is [`load_mp4_from_bytes`], with a native-only `load_mp4`
//! convenience wrapper that reads the bytes from a path on disk.

mod asset;
mod config;
mod error;

pub use config::{Mode, Mp4Config};
pub use error::Mp4Error;

use re_chunk::{Chunk, EntityPath};

/// Load an mp4 file from disk and return an iterator of chunks.
///
/// See [`Mp4Config`] for the available modes. The iterator may yield `Err`
/// items for individual chunk failures; callers may skip them.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_mp4(
    path: &std::path::Path,
    config: &Mp4Config,
    entity_path: &EntityPath,
) -> Result<Mp4ChunkIter, Mp4Error> {
    re_tracing::profile_function!();
    let bytes = std::fs::read(path)?;
    load_mp4_from_bytes(bytes, config, entity_path)
}

/// Load mp4 bytes from memory and return an iterator of chunks.
///
/// See [`Mp4ChunkIter`] for details on the returned iterator.
pub fn load_mp4_from_bytes(
    bytes: Vec<u8>,
    config: &Mp4Config,
    entity_path: &EntityPath,
) -> Result<Mp4ChunkIter, Mp4Error> {
    re_tracing::profile_function!();
    match &config.mode {
        Mode::Asset { timepoint } => Ok(Mp4ChunkIter(Mp4ChunkIterInner::Asset(
            asset::AssetChunkIter::new(
                bytes,
                entity_path,
                config.timeline_name,
                timepoint.clone(),
            )?,
        ))),
    }
}

/// Lazy iterator returned by [`load_mp4_from_bytes`] (and the native-only `load_mp4`).
///
/// Chunks are constructed one at a time in [`Iterator::next`]. The item type is
/// `Result<Chunk, Mp4Error>` because constructing an individual chunk can fail
/// mid-stream; it is up to the caller to decide whether to abort or skip on
/// `Err`. Note that *unreadable frame timestamps* are not surfaced as an `Err`
/// item — that case is handled leniently by emitting only the asset chunk.
pub struct Mp4ChunkIter(Mp4ChunkIterInner);

enum Mp4ChunkIterInner {
    Asset(asset::AssetChunkIter),
}

impl Iterator for Mp4ChunkIter {
    type Item = Result<Chunk, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            Mp4ChunkIterInner::Asset(it) => it.next(),
        }
    }
}
