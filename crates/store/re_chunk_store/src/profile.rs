use crate::ChunkStoreConfig;

/// Named optimization profile combining chunk-size thresholds with
/// post-processing knobs (extra passes, GoP rebatching, thick/thin split).
///
/// Two presets are provided: [`Self::LIVE`] (small chunks tuned for the live
/// Viewer workflow) and [`Self::OBJECT_STORE`] (large chunks tuned for
/// object-store-backed query and streaming).
///
/// A profile does not consult environment variables. Callers that need env-var
/// layering must call [`ChunkStoreConfig::apply_env`] themselves on the result
/// of [`Self::to_chunk_store_config`].
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizationProfile {
    pub chunk_max_bytes: u64,
    pub chunk_max_rows: u64,
    pub chunk_max_rows_if_unsorted: u64,
    pub num_extra_passes: u32,
    pub gop_batching: bool,
    pub split_size_ratio: Option<f64>,
}

impl OptimizationProfile {
    /// Optimized for the live Viewer workflow: small chunks for low-latency
    /// rendering and fine-grained time-panel precision.
    ///
    /// Threshold values intentionally mirror [`ChunkStoreConfig::DEFAULT`].
    /// If you change one, change the other (see the unit test in this module).
    pub const LIVE: Self = Self {
        chunk_max_bytes: 12 * 8 * 4096,
        chunk_max_rows: 4096,
        chunk_max_rows_if_unsorted: 1024,
        num_extra_passes: 50,
        gop_batching: true,
        split_size_ratio: None,
    };

    /// Optimized for object-store-backed storage (e.g. a catalog server):
    /// larger chunks tuned for query throughput and streaming over the network.
    pub const OBJECT_STORE: Self = Self {
        chunk_max_bytes: 2 * 1024 * 1024,
        chunk_max_rows: 65_536,
        chunk_max_rows_if_unsorted: 8_192,
        num_extra_passes: 50,
        gop_batching: true,
        split_size_ratio: None,
    };

    /// Build a [`ChunkStoreConfig`] from this profile, with `enable_changelog`
    /// at its `ChunkStoreConfig::DEFAULT` value.
    ///
    /// Headless callers (the CLI, the Python `LazyChunkStream.collect`
    /// binding) want the changelog off and must set it explicitly on the
    /// returned config.
    pub fn to_chunk_store_config(&self) -> ChunkStoreConfig {
        ChunkStoreConfig {
            enable_changelog: ChunkStoreConfig::DEFAULT.enable_changelog,
            chunk_max_bytes: self.chunk_max_bytes,
            chunk_max_rows: self.chunk_max_rows,
            chunk_max_rows_if_unsorted: self.chunk_max_rows_if_unsorted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression detector: if someone changes `ChunkStoreConfig::DEFAULT`
    /// thresholds without updating `LIVE`, this fails.
    #[test]
    fn live_thresholds_track_default() {
        assert_eq!(
            OptimizationProfile::LIVE.chunk_max_bytes,
            ChunkStoreConfig::DEFAULT.chunk_max_bytes,
        );
        assert_eq!(
            OptimizationProfile::LIVE.chunk_max_rows,
            ChunkStoreConfig::DEFAULT.chunk_max_rows,
        );
        assert_eq!(
            OptimizationProfile::LIVE.chunk_max_rows_if_unsorted,
            ChunkStoreConfig::DEFAULT.chunk_max_rows_if_unsorted,
        );
    }

    #[test]
    fn to_chunk_store_config_carries_thresholds() {
        let cfg = OptimizationProfile::OBJECT_STORE.to_chunk_store_config();
        assert_eq!(cfg.chunk_max_bytes, 2 * 1024 * 1024);
        assert_eq!(cfg.chunk_max_rows, 65_536);
        assert_eq!(cfg.chunk_max_rows_if_unsorted, 8_192);
        assert_eq!(
            cfg.enable_changelog,
            ChunkStoreConfig::DEFAULT.enable_changelog
        );
    }
}
