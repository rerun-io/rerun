//! Environment configuration for dataset scans: which pipeline runs them,
//! and the v2 pipeline's flow-control knobs.
//!
//! Read once per query, at plan construction.

use datafusion::common::{Result, config_datafusion_err};

const ENV_PIPELINE: &str = "RERUN_DATAFRAME_PIPELINE";
const ENV_FETCH_CONCURRENCY: &str = "RERUN_DATAFRAME_FETCH_CONCURRENCY";
const ENV_WINDOW_BYTES: &str = "RERUN_DATAFRAME_WINDOW_BYTES";

/// `N` default inherits v1's measured `IO_PIPELINE_BUFFER`.
const DEFAULT_FETCH_CONCURRENCY: usize = 24;

/// `W` default: covers `N` in-flight 8 MiB targets after decode expansion
/// while bounding successor-segment prefetch.
const DEFAULT_WINDOW_BYTES: u64 = 512 * 1024 * 1024;

/// Which pipeline executes dataset scans, from `RERUN_DATAFRAME_PIPELINE`.
///
/// Unset or `v1` selects the IO-loop / CPU-worker pipeline; `v2` selects
/// the streaming pipeline (`PIPELINE_V2.md`). Case and surrounding
/// whitespace are ignored; any other value is an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataframePipeline {
    /// The IO-loop / CPU-worker pipeline, the default.
    V1,

    /// The streaming pipeline, with its flow-control knobs.
    V2(FetchConfig),
}

/// The v2 pipeline's two flow-control knobs:
/// `N` concurrent fetches, `W` bytes of issued-but-not-yet-released
/// estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchConfig {
    /// `N`: how many planned batches are in flight at once (`buffered(N)`).
    /// From `RERUN_DATAFRAME_FETCH_CONCURRENCY`, an integer ≥ 1.
    pub fetch_concurrency: usize,

    /// `W`: the issuance window in estimated-decoded bytes.
    /// From `RERUN_DATAFRAME_WINDOW_BYTES`, a byte size such as `512MiB` or a
    /// bare integer, between 1 byte and `u32::MAX` bytes (the window counts
    /// permits in a `u32`).
    pub window_bytes: u64,
}

impl DataframePipeline {
    /// Reads `RERUN_DATAFRAME_PIPELINE`, and the v2 knobs only when it
    /// selects v2.
    ///
    /// Returns a configuration error for any invalid value instead of
    /// falling back to a default.
    pub fn from_env() -> Result<Self> {
        Self::parse(read_env(ENV_PIPELINE)?.as_deref(), || {
            FetchConfig::parse(
                read_env(ENV_FETCH_CONCURRENCY)?.as_deref(),
                read_env(ENV_WINDOW_BYTES)?.as_deref(),
            )
        })
    }

    /// `fetch_config` runs only when v2 is selected, so the v2 knobs are
    /// neither read nor validated for v1.
    fn parse(
        pipeline: Option<&str>,
        fetch_config: impl FnOnce() -> Result<FetchConfig>,
    ) -> Result<Self> {
        match pipeline.map(str::to_ascii_lowercase).as_deref() {
            None | Some("v1") => Ok(Self::V1),
            Some("v2") => Ok(Self::V2(fetch_config()?)),
            Some(_) => Err(config_datafusion_err!(
                "{ENV_PIPELINE} must be \"v1\" or \"v2\", got {pipeline:?}"
            )),
        }
    }
}

impl FetchConfig {
    fn parse(fetch_concurrency: Option<&str>, window_bytes: Option<&str>) -> Result<Self> {
        let fetch_concurrency = match fetch_concurrency {
            None => DEFAULT_FETCH_CONCURRENCY,
            Some(raw) => raw
                .parse::<usize>()
                .ok()
                .filter(|&n| n >= 1)
                .ok_or_else(|| {
                    config_datafusion_err!(
                        "{ENV_FETCH_CONCURRENCY} must be an integer >= 1, got {raw:?}"
                    )
                })?,
        };

        let window_bytes = match window_bytes {
            None => DEFAULT_WINDOW_BYTES,
            Some(raw) => re_format::parse_bytes(raw)
                .or_else(|| raw.parse::<i64>().ok())
                .and_then(|n| u64::try_from(n).ok())
                .filter(|&n| (1..=u64::from(u32::MAX)).contains(&n))
                .ok_or_else(|| {
                    config_datafusion_err!(
                        "{ENV_WINDOW_BYTES} must be a byte size of at least 1 B and below 4 GiB \
                         (e.g. \"512MiB\" or a bare integer), got {raw:?}"
                    )
                })?,
        };

        Ok(Self {
            fetch_concurrency,
            window_bytes,
        })
    }
}

/// A set, non-blank environment variable, trimmed. Unset and blank both
/// read as `None`; a value that is not valid Unicode is an error.
fn read_env(key: &str) -> Result<Option<String>> {
    match std::env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(config_datafusion_err!(
            "{key} is set to a value that is not valid Unicode"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_pipeline_is_an_error() {
        let err =
            DataframePipeline::parse(Some("v3"), || FetchConfig::parse(None, None)).unwrap_err();
        assert!(
            err.to_string().contains("RERUN_DATAFRAME_PIPELINE"),
            "error should name the variable, got: {err}",
        );
    }

    #[test]
    fn v2_reads_its_knobs_and_defaults_the_rest() {
        assert_eq!(
            DataframePipeline::parse(Some("V2"), || FetchConfig::parse(None, Some("256MiB")))
                .unwrap(),
            DataframePipeline::V2(FetchConfig {
                fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
                window_bytes: 256 * 1024 * 1024,
            }),
        );
    }

    #[test]
    fn invalid_v2_knobs_are_errors() {
        for (fetch_concurrency, window_bytes) in [
            (Some("0"), None),
            (Some("many"), None),
            (None, Some("0")),
            (None, Some("-1")),
            (None, Some("4GiB")),
            (None, Some("8GiB")),
            (None, Some("lots")),
        ] {
            assert!(
                FetchConfig::parse(fetch_concurrency, window_bytes).is_err(),
                "fetch_concurrency={fetch_concurrency:?} window_bytes={window_bytes:?} must be rejected",
            );
        }
    }

    #[test]
    fn v1_ignores_v2_knobs() {
        assert_eq!(
            DataframePipeline::parse(None, || FetchConfig::parse(Some("0"), Some("lots"))).unwrap(),
            DataframePipeline::V1,
        );
    }
}
