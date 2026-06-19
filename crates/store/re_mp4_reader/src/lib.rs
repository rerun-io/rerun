//! Core mp4-to-chunk loading logic for Rerun.
//!
//! Reads any `.mp4` file (or its in-memory bytes) and emits an iterator of
//! [`re_chunk::Chunk`]s ready to be sent to a recording or chunk store.
//!
//! Two output modes are supported via [`Mode`]:
//! - [`Mode::Asset`] — emit an [`re_sdk_types::archetypes::AssetVideo`] blob
//!   chunk plus a [`re_sdk_types::archetypes::VideoFrameReference`] index chunk.
//! - [`Mode::Stream`] — demux the mp4 with [`re_video`] and emit a static
//!   [`re_sdk_types::archetypes::VideoStream`] codec chunk followed by per-GOP
//!   (or per-sample) `VideoSample` / `IsKeyframe` chunks.
//!
//! The entry point is [`load_mp4_from_bytes`], with a native-only `load_mp4`
//! convenience wrapper that reads the bytes from a path on disk.

mod asset;
mod config;
mod error;
mod stream;

pub use config::{Mode, Mp4Config};
pub use error::Mp4Error;

use itertools::Either;

use re_chunk::{Chunk, EntityPath};

/// Load an mp4 file from disk and return an iterator of chunks.
///
/// See [`Mp4Config`] for the available modes. The iterator may yield `Err`
/// items for individual chunk failures; callers may skip them.
///
/// In [`Mode::Stream`], chunks are produced by streaming the file from disk —
/// only the container metadata and one GOP's worth of sample bytes are resident
/// at a time, so the whole video is never loaded into memory. [`Mode::Asset`]
/// still reads the whole file, since it logs the mp4 as a single blob.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_mp4(
    path: &std::path::Path,
    config: &Mp4Config,
    entity_path: &EntityPath,
) -> Result<impl Iterator<Item = Result<Chunk, Mp4Error>> + use<>, Mp4Error> {
    re_tracing::profile_function!();
    let debug_name = path.display().to_string();
    match &config.mode {
        Mode::Asset { timepoint } => Ok(Either::Left(new_asset_iter(
            std::fs::read(path)?,
            config,
            entity_path,
            timepoint.clone(),
        )?)),

        Mode::Stream {
            chunk_by_gop,
            allow_b_frames,
        } => {
            let file = std::fs::File::open(path)?;
            let size = file.metadata()?.len();
            let iter = stream::iter_chunks(
                std::io::BufReader::new(file),
                size,
                entity_path,
                &config.timeline_name,
                *chunk_by_gop,
                config.timeline_type,
                *allow_b_frames,
                &debug_name,
            )?;
            Ok(Either::Right(iter))
        }
    }
}

/// Load mp4 bytes from memory and return an iterator of chunks.
///
/// The returned iterator is lazy: chunks are constructed one at a time as it is
/// drained. The item type is `Result<Chunk, Mp4Error>` because constructing an
/// individual chunk can fail mid-stream; it is up to the caller to decide
/// whether to abort or skip on `Err`. Note that *unreadable frame timestamps*
/// are not surfaced as an `Err` item — that case is handled leniently by
/// emitting only the asset chunk.
///
/// `debug_name` is a human-readable label for the video (e.g. its source path
/// or URL) used only in log and panic messages — it has no effect on decoding.
///
/// Unlike the path-based [`load_mp4`], the bytes are already in memory here, so
/// [`Mode::Stream`] reads samples from the in-memory buffer rather than streaming
/// from disk.
pub fn load_mp4_from_bytes(
    bytes: Vec<u8>,
    config: &Mp4Config,
    entity_path: &EntityPath,
    debug_name: &str,
) -> Result<impl Iterator<Item = Result<Chunk, Mp4Error>> + use<>, Mp4Error> {
    re_tracing::profile_function!();
    match &config.mode {
        Mode::Asset { timepoint } => Ok(Either::Left(new_asset_iter(
            bytes,
            config,
            entity_path,
            timepoint.clone(),
        )?)),

        Mode::Stream {
            chunk_by_gop,
            allow_b_frames,
        } => {
            let size = bytes.len() as u64;
            let iter = stream::iter_chunks(
                std::io::Cursor::new(bytes),
                size,
                entity_path,
                &config.timeline_name,
                *chunk_by_gop,
                config.timeline_type,
                *allow_b_frames,
                debug_name,
            )?;
            Ok(Either::Right(iter))
        }
    }
}

fn new_asset_iter(
    bytes: Vec<u8>,
    config: &Mp4Config,
    entity_path: &EntityPath,
    timepoint: re_chunk::TimePoint,
) -> Result<asset::AssetChunkIter, Mp4Error> {
    asset::AssetChunkIter::new(
        bytes,
        entity_path,
        config.timeline_name,
        config.timeline_type,
        timepoint,
    )
}
