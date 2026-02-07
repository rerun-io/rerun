use std::str::FromStr;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
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

    #[error("Dataset data URL requires a `?segment_id` query parameter")]
    MissingSegmentId,

    #[error(
        "Dataset data URL cannot contain both `?segment_id` and legacy `?partition_id` query parameters"
    )]
    AmbiguousSegmentId,

    #[error("Invalid TUID: {0}")]
    InvalidTuid(<re_tuid::Tuid as FromStr>::Err),
}
