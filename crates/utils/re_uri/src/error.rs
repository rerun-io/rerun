use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] url::ParseError),

    #[error(
        "Invalid or missing scheme (expected one of: `rerun://`, `rerun+http://`, `rerun+https://`)"
    )]
    InvalidScheme,

    #[error("Invalid time range (expected `TIMELINE@time..time`): {0}")]
    InvalidTimeRange(String),

    #[error("Unexpected URI:: {0}")]
    UnexpectedUri(String),

    #[error("Unexpected opaque origin: {0}")]
    UnexpectedOpaqueOrigin(String),

    #[error("Unexpected base URL: {0}")]
    UnexpectedBaseUrl(String),

    #[error("URL {url:?} cannot be loaded as a recording")]
    CannotLoadUrlAsRecording { url: String },

    #[error("Dataset data URL required a `?partition_id` query parameter")]
    MissingPartitionId,

    #[error("Invalid TUID: {0}")]
    InvalidTuid(<re_tuid::Tuid as FromStr>::Err),
}
