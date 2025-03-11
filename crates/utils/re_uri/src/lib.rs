//! Rerun uses its own URL scheme to access information across the network.
//!
//! The following schemes are supported: `rerun+http://`, `rerun+https://` and
//! `rerun://`, which is an alias for `rerun+https://`. These schemes are then
//! converted on the fly to either `http://` or `https://`. Rerun uses gRPC-based
//! protocols under the hood, which means that the paths (`/catalog`,
//! `/recording/12345`, …) are mapped to gRPC services and methods on the fly.
//!
//! <div class="warning">
//! In most cases locally running instances of Rerun will not have proper TLS
//! configuration. In these cases, the `rerun+http://` scheme can be used. Naturally,
//! this means that the underlying connection will not be encrypted.
//! </div>
//!
//! The following are examples of valid Rerun URIs:
//!
//! ```
//! for uri in [
//!     // Access the dataplatform catalog.
//!     "rerun://rerun.io",
//!     "rerun://rerun.io:51234/catalog",
//!     "rerun+http://localhost:51234/catalog",
//!     "rerun+https://localhost:51234/catalog",
//!
//!     // Proxy to send messages to another viewer.
//!     "rerun+http://localhost:51234/proxy",
//!
//!     // Links to recording on the dataplatform (optionally with timestamp).
//!     "rerun://127.0.0.1:1234/recording/12345?time_range=timeline@10s..20s"
//! ] {
//!     assert!(uri.parse::<re_uri::RedapUri>().is_ok());
//! }
//!
//! ```

mod endpoints;
mod error;

use re_log_types::{IndexCell, TimeInt};

pub use self::{
    endpoints::{catalog::CatalogEndpoint, proxy::ProxyEndpoint, recording::RecordingEndpoint},
    error::Error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRange {
    pub timeline: re_log_types::Timeline,
    pub range: re_log_types::ResolvedTimeRangeF,
}

impl TimeRange {
    const QUERY_KEY: &str = "time_range";
}

impl std::fmt::Display for TimeRange {
    /// Used for formatting time ranges in URLs
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { timeline, range } = self;

        let min = IndexCell::new(timeline.typ(), range.min.floor());
        let max = IndexCell::new(timeline.typ(), range.max.ceil());

        let name = timeline.name();
        write!(f, "{name}@{min}..{max}")
    }
}

impl TryFrom<&str> for TimeRange {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (timeline, range) = value
            .split_once('@')
            .ok_or_else(|| Error::InvalidTimeRange("Missing @".to_owned()))?;

        let (min, max) = range
            .split_once("..")
            .ok_or_else(|| Error::InvalidTimeRange("Missing ..".to_owned()))?;

        let min = min.parse::<IndexCell>().map_err(|err| {
            Error::InvalidTimeRange(format!("Failed to parse time index '{min}': {err}"))
        })?;
        let max = max.parse::<IndexCell>().map_err(|err| {
            Error::InvalidTimeRange(format!("Failed to parse time index '{max}': {err}"))
        })?;

        if min.typ() != max.typ() {
            return Err(Error::InvalidTimeRange(
                format!("min/max had differing types. Min was identified as {}, whereas max was identified as {}", min.typ(), max.typ()),
            ));
        }

        let timeline = re_log_types::Timeline::new(timeline, min.typ());
        let range = re_log_types::ResolvedTimeRangeF::new(TimeInt::from(min), TimeInt::from(max));

        Ok(Self { timeline, range })
    }
}

/// The different schemes supported by Rerun.
///
/// We support `rerun`, `rerun+http`, and `rerun+https`.
#[derive(
    Debug, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Scheme {
    Rerun,
    RerunHttp,
    RerunHttps,
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rerun => write!(f, "rerun"),
            Self::RerunHttp => write!(f, "rerun+http"),
            Self::RerunHttps => write!(f, "rerun+https"),
        }
    }
}

impl Scheme {
    /// Converts a [`Scheme`] to either `http` or `https`.
    fn as_http_scheme(&self) -> &str {
        match self {
            Self::Rerun | Self::RerunHttps => "https",
            Self::RerunHttp => "http",
        }
    }

    /// Converts a rerun url into a canonical http or https url.
    fn canonical_url(&self, url: &str) -> String {
        match self {
            Self::Rerun => {
                debug_assert!(url.starts_with("rerun://"));
                url.replace("rerun://", "https://")
            }
            Self::RerunHttp => {
                debug_assert!(url.starts_with("rerun+http://"));
                url.replace("rerun+http://", "http://")
            }
            Self::RerunHttps => {
                debug_assert!(url.starts_with("rerun+https://"));
                url.replace("rerun+https://", "https://")
            }
        }
    }
}

impl TryFrom<&str> for Scheme {
    type Error = Error;

    fn try_from(url: &str) -> Result<Self, Self::Error> {
        if url.starts_with("rerun://") {
            Ok(Self::Rerun)
        } else if url.starts_with("rerun+http://") {
            Ok(Self::RerunHttp)
        } else if url.starts_with("rerun+https://") {
            Ok(Self::RerunHttps)
        } else {
            Err(Self::Error::InvalidScheme)
        }
    }
}

#[derive(
    Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Origin {
    pub scheme: Scheme,
    pub host: url::Host<String>,
    pub port: u16,
}

impl Origin {
    /// Converts the [`Origin`] to a URL that starts with either `http` or `https`.
    pub fn as_url(&self) -> String {
        format!(
            "{}://{}:{}",
            self.scheme.as_http_scheme(),
            self.host,
            self.port
        )
    }

    /// Converts the [`Origin`] to a `http` URL.
    ///
    /// In most cases you want to use [`Origin::as_url()`] instead.
    pub fn coerce_http_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Parses a URL and returns the [`Origin`] and the canonical URL (i.e. one that
///  starts with `http://` or `https://`).
fn replace_and_parse(value: &str) -> Result<(Origin, url::Url), Error> {
    let scheme = Scheme::try_from(value)?;
    let rewritten = scheme.canonical_url(value);

    // We have to first rewrite the endpoint, because `Url` does not allow
    // `.set_scheme()` for non-opaque origins, nor does it return a proper
    // `Origin` in that case.
    let http_url = url::Url::parse(&rewritten)?;

    let url::Origin::Tuple(_, host, port) = http_url.origin() else {
        return Err(Error::UnexpectedOpaqueOrigin(value.to_owned()));
    };

    let origin = Origin { scheme, host, port };

    Ok((origin, http_url))
}

impl TryFrom<&str> for Origin {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        replace_and_parse(value).map(|(origin, _)| origin)
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RedapUri {
    Recording(RecordingEndpoint),
    Catalog(CatalogEndpoint),

    /// We use the `/proxy` endpoint to access another _local_ viewer.
    Proxy(ProxyEndpoint),
}

impl std::fmt::Display for RedapUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording(endpoint) => write!(f, "{endpoint}",),
            Self::Catalog(endpoint) => write!(f, "{endpoint}",),
            Self::Proxy(endpoint) => write!(f, "{endpoint}",),
        }
    }
}

impl std::str::FromStr for RedapUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl TryFrom<&str> for RedapUri {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (origin, http_url) = replace_and_parse(value)?;

        // :warning: We limit the amount of segments, which might need to be
        // adjusted when adding additional resources.
        let segments = http_url
            .path_segments()
            .ok_or_else(|| Error::UnexpectedBaseUrl(value.to_owned()))?
            .take(2)
            .filter(|s| !s.is_empty()) // handle trailing slashes
            .collect::<Vec<_>>();

        let time_range = http_url
            .query_pairs()
            .find(|(key, _)| key == TimeRange::QUERY_KEY)
            .map(|(_, value)| TimeRange::try_from(value.as_ref()));

        match segments.as_slice() {
            ["recording", recording_id] => Ok(Self::Recording(RecordingEndpoint::new(
                origin,
                (*recording_id).to_owned(),
                time_range.transpose()?,
            ))),
            ["proxy"] => Ok(Self::Proxy(ProxyEndpoint::new(origin))),
            ["catalog"] | [] => Ok(Self::Catalog(CatalogEndpoint::new(origin))),
            [unknown, ..] => Err(Error::UnexpectedEndpoint(format!("{unknown}/"))),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use core::net::Ipv4Addr;

    #[test]
    fn scheme_conversion() {
        assert_eq!(Scheme::Rerun.as_http_scheme(), "https");
        assert_eq!(Scheme::RerunHttp.as_http_scheme(), "http");
        assert_eq!(Scheme::RerunHttps.as_http_scheme(), "https");
    }

    #[test]
    fn origin_conversion() {
        let origin = Origin {
            scheme: Scheme::Rerun,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "https://127.0.0.1:1234");

        let origin = Origin {
            scheme: Scheme::RerunHttp,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "http://127.0.0.1:1234");

        let origin = Origin {
            scheme: Scheme::RerunHttps,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "https://127.0.0.1:1234");
    }

    #[test]
    fn test_recording_url_to_address() {
        let url = "rerun://127.0.0.1:1234/recording/12345";
        let address: RedapUri = url.try_into().unwrap();

        let RedapUri::Recording(RecordingEndpoint {
            origin,
            recording_id,
            time_range,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(recording_id, "12345");
        assert_eq!(time_range, None);
    }

    #[test]
    fn test_recording_url_time_range_sequence_to_address() {
        let url = "rerun://127.0.0.1:1234/recording/12345?time_range=timeline@100..200";
        let address: RedapUri = url.try_into().unwrap();

        let RedapUri::Recording(RecordingEndpoint {
            origin,
            recording_id,
            time_range,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(recording_id, "12345");
        assert_eq!(
            time_range,
            Some(TimeRange {
                timeline: re_log_types::Timeline::new_sequence("timeline"),
                range: re_log_types::ResolvedTimeRangeF::new(
                    re_log_types::TimeReal::from(100.0),
                    re_log_types::TimeReal::from(200.0)
                )
            })
        );
    }

    #[test]
    fn test_recording_url_time_range_timepoint_to_address() {
        let url = "rerun://127.0.0.1:1234/recording/12345?time_range=log_time@2022-01-01T00:00:03.123456789Z..2022-01-01T00:00:13.123456789Z";
        let address: RedapUri = url.try_into().unwrap();

        let RedapUri::Recording(RecordingEndpoint {
            origin,
            recording_id,
            time_range,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(recording_id, "12345");
        assert_eq!(
            time_range,
            Some(TimeRange {
                timeline: re_log_types::Timeline::new_timestamp("log_time"),
                range: re_log_types::ResolvedTimeRangeF::new(
                    re_log_types::TimeInt::from_nanos(
                        1_640_995_203_123_456_789.try_into().unwrap()
                    ),
                    re_log_types::TimeInt::from_nanos(
                        1_640_995_213_123_456_789.try_into().unwrap()
                    ),
                )
            })
        );
    }

    #[test]
    fn test_recording_url_time_range_temporal() {
        for url in [
            "rerun://127.0.0.1:1234/recording/12345?time_range=timeline@1.23s..72s",
            "rerun://127.0.0.1:1234/recording/12345?time_range=timeline@1230ms..1m12s",
        ] {
            let address: RedapUri = url.try_into().unwrap();

            let RedapUri::Recording(RecordingEndpoint {
                origin,
                recording_id,
                time_range,
            }) = address
            else {
                panic!("Expected recording");
            };

            assert_eq!(origin.scheme, Scheme::Rerun);
            assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
            assert_eq!(origin.port, 1234);
            assert_eq!(recording_id, "12345");
            assert_eq!(
                time_range,
                Some(TimeRange {
                    timeline: re_log_types::Timeline::new_duration("timeline"),
                    range: re_log_types::ResolvedTimeRangeF::new(
                        re_log_types::TimeReal::from_seconds(1.23),
                        re_log_types::TimeReal::from_seconds(72.0)
                    )
                })
            );
        }
    }

    #[test]
    fn test_http_catalog_url_to_address() {
        let url = "rerun+http://127.0.0.1:50051/catalog";
        let address: RedapUri = url.try_into().unwrap();
        assert!(matches!(
            address,
            RedapUri::Catalog(CatalogEndpoint {
                origin: Origin {
                    scheme: Scheme::RerunHttp,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
                },
            })
        ));
    }

    #[test]
    fn test_https_catalog_url_to_address() {
        let url = "rerun+https://127.0.0.1:50051/catalog";
        let address: RedapUri = url.try_into().unwrap();

        assert!(matches!(
            address,
            RedapUri::Catalog(CatalogEndpoint {
                origin: Origin {
                    scheme: Scheme::RerunHttps,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
                }
            })
        ));
    }

    #[test]
    fn test_localhost_url() {
        let url = "rerun+http://localhost:51234/catalog";
        let address = RedapUri::try_from(url).unwrap();

        assert_eq!(
            address,
            RedapUri::Catalog(CatalogEndpoint {
                origin: Origin {
                    scheme: Scheme::RerunHttp,
                    host: url::Host::<String>::Domain("localhost".to_owned()),
                    port: 51234
                }
            })
        );
    }

    #[test]
    fn test_invalid_url() {
        let url = "http://wrong-scheme:1234/recording/12345";
        let address: Result<RedapUri, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::Error::InvalidScheme { .. }
        ));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapUri, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::Error::UnexpectedEndpoint(unknown) if &unknown == "redap/"
        ));
    }

    #[test]
    fn test_proxy_endpoint() {
        let url = "rerun://localhost:51234/proxy";
        let address: Result<RedapUri, _> = url.try_into();

        let expected = RedapUri::Proxy(ProxyEndpoint {
            origin: Origin {
                scheme: Scheme::Rerun,
                host: url::Host::Domain("localhost".to_owned()),
                port: 51234,
            },
        });

        assert_eq!(address.unwrap(), expected);

        let url = "rerun://localhost:51234/proxy/";
        let address: Result<RedapUri, _> = url.try_into();

        assert_eq!(address.unwrap(), expected);
    }

    #[test]
    fn test_catalog_default() {
        let url = "rerun://localhost:51234";
        let address: Result<RedapUri, _> = url.try_into();

        let expected = RedapUri::Catalog(CatalogEndpoint {
            origin: Origin {
                scheme: Scheme::Rerun,
                host: url::Host::Domain("localhost".to_owned()),
                port: 51234,
            },
        });

        assert_eq!(address.unwrap(), expected);

        let url = "rerun://localhost:51234/";
        let address: Result<RedapUri, _> = url.try_into();

        assert_eq!(address.unwrap(), expected);
    }
}
