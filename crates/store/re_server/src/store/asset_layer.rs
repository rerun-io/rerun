#![expect(dead_code)] // TODO(RR-4755): enable registration of asset layers

use re_types_core::LayerName;

use crate::store::Source;

/// Some data that is shared by the entire dataset.
///
/// For instance: all segments in a dataset share the same URDF,
/// so we put it in a single `.rrd` and register it as an "asset layer".
/// On queries, we act as if that `.rrd` is part of every segment, even though it's only stored once.
///
/// This means dataset queries is likely to return the same `ChunkId`:s from the asset layer
/// multiple times for each segment, and it is up to the client to only download
/// each chunk once.
pub struct AssetLayer {
    pub name: LayerName,
    pub source: Source,
}
