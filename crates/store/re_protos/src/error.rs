//! Machine-readable error identities carried by `google.rpc.ErrorInfo`.

/// Domain identifying Rerun errors independently of the server's address.
pub const ERROR_DOMAIN: &str = "rerun.io";

/// The request was rejected before reading because the replica is behind the watermark.
pub const DATASET_REVISION_BEHIND: &str = "DATASET_REVISION_BEHIND";

/// The request names an entity path or timeline the dataset's chunk indexing policy excludes.
pub const CHUNK_NOT_INDEXED: &str = "CHUNK_NOT_INDEXED";

/// A full-scope request touches data the chunk indexing policy excludes, but selects all segments.
pub const UNINDEXED_DATA_NEEDS_SEGMENTS: &str = "UNINDEXED_DATA_NEEDS_SEGMENTS";

/// Reading unindexed data for the requested segments would exceed the server's footer limits.
pub const UNINDEXED_DATA_TOO_LARGE: &str = "UNINDEXED_DATA_TOO_LARGE";
