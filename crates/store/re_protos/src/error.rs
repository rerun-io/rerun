//! Machine-readable error identities carried by `google.rpc.ErrorInfo`.

/// Domain identifying Rerun errors independently of the server's address.
pub const ERROR_DOMAIN: &str = "rerun.io";

/// The request was rejected before reading because the replica is behind the watermark.
pub const DATASET_REVISION_BEHIND: &str = "DATASET_REVISION_BEHIND";
