#![allow(clippy::iter_over_hash_type)]

//! Indexes of Rerun chunks, independent of how the chunks are stored.
//!
//! An index describes which chunks exist in a recording and what each of them contains
//! (entity path, timelines, components, time ranges, row counts), without holding the chunk data
//! itself.
//! That is what makes it possible to answer relevancy queries (latest-at, range, dataframe) and
//! to fetch only the chunks that matter.
//!
//! * [`RawRrdManifest`] is the index as it is stored and transported: one Arrow record batch per
//!   recording, with one row per chunk. [`RrdManifestBuilder`] produces it from chunks.
//! * [`RrdManifest`] is the validated, pre-parsed form that the viewer and the servers work with.
//! * [`ChunkProvider`] pairs an index with a way to load the chunks it describes.
//!   [`InMemoryChunkProvider`] serves chunks that are already in memory.
//!
//! The manifest records where each chunk lives as a byte offset and size, but this crate never
//! reads or writes those bytes. Encoding and decoding RRD streams, and reading footers and chunks
//! out of `.rrd` files, is the job of `re_log_encoding`, which builds on this crate.

mod chunk_provider;
mod error;
mod raw_rrd_manifest;
mod rrd_manifest;
mod rrd_manifest_builder;

pub use self::chunk_provider::{ChunkProvider, ChunkProviderError, InMemoryChunkProvider};
pub use self::error::{ChunkIndexError, ChunkIndexResult};
pub use self::raw_rrd_manifest::{
    RawRrdManifest, RrdManifestSha256, RrdManifestStaticMap, RrdManifestTemporalMap,
    RrdManifestTemporalMapEntry, sha256_to_hex,
};
pub use self::rrd_manifest::RrdManifest;
pub use self::rrd_manifest_builder::RrdManifestBuilder;
