use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] url::ParseError),

    #[error("invalid or missing scheme (expected `rerun(+http|+https)://`)")]
    InvalidScheme,

    #[error("invalid time range (expected `TIMELINE@time..time`): {0}")]
    InvalidTimeRange(String),

    #[error("unexpected endpoint: {0}")]
    UnexpectedEndpoint(String),

    #[error("unexpected opaque origin: {0}")]
    UnexpectedOpaqueOrigin(String),

    #[error("unexpected base URL: {0}")]
    UnexpectedBaseUrl(String),

    #[error("URL {url:?} cannot be loaded as a recording")]
    CannotLoadUrlAsRecording { url: String },

    #[error("Dataset data URL required a `?partition_id` query parameter")]
    MissingPartitionId,

    #[error("invalid TUID: {0}")]
    InvalidTuid(<re_tuid::Tuid as FromStr>::Err),
}
