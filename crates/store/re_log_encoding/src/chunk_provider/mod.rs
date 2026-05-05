use std::sync::Arc;

use re_chunk::{Chunk, ChunkId};

use crate::{RawRrdManifest, RrdManifest};

#[cfg(feature = "decoder")]
#[cfg(not(target_arch = "wasm32"))]
mod rrd;

#[cfg(feature = "decoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use self::rrd::RrdChunkProvider;

/// Synchronous backend that exposes an indexed chunk source.
///
/// A provider serves both the **index** (which chunks exist and their metadata) and the **bytes**
/// (the chunks themselves).
///
/// ## Contract
///
/// - All `ids` in [`Self::load_chunks`] must be known to the provider (i.e. root chunks
///   present in [`Self::manifest`]). Unknown ids produce an error and abort the batch.
/// - The returned `Vec<Arc<Chunk>>` may be in any order; callers must not rely on input ordering.
/// - [`Self::manifest`] and [`Self::raw_manifest`] are stable for the lifetime of the provider —
///   they never change once constructed.
pub trait ChunkProvider: Send + Sync {
    /// The validated, indexed manifest of chunks this provider serves.
    fn manifest(&self) -> &Arc<RrdManifest>;

    /// The raw, as-parsed manifest. Kept around so consumers (e.g. the server's `GetRrdManifest`
    /// handler) can re-serialize without re-validating.
    fn raw_manifest(&self) -> &Arc<RawRrdManifest>;

    /// Human-readable identifier for this provider's backing source, used in diagnostic messages
    /// (e.g. an RRD path or a segment id). Not parsed; format is purely for display.
    fn source(&self) -> String;

    /// Load chunks by id.
    fn load_chunks(&self, ids: &[ChunkId]) -> Result<Vec<Arc<Chunk>>, ChunkProviderError>;
}

/// Provider-level error.
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct ChunkProviderError(pub Box<dyn std::error::Error + Send + Sync + 'static>);
