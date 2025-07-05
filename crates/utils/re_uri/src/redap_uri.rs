use std::str::FromStr as _;

use re_log_types::StoreId;

use crate::{
    CatalogUri, DEFAULT_PROXY_PORT, DEFAULT_REDAP_PORT, DatasetDataUri, EntryUri, Error, Fragment,
    Origin, ProxyUri,
};

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[cfg_attr(not(target_arch = "wasm32"), expect(clippy::large_enum_variant))]
pub enum RedapUri {
    /// `/catalog` - also the default if there is no /endpoint
    Catalog(CatalogUri),

    /// `/entry`
    Entry(EntryUri),

    /// `/dataset`
    DatasetData(DatasetDataUri),

    /// We use the `/proxy` endpoint to access another _local_ viewer.
    Proxy(ProxyUri),
}

impl RedapUri {
    pub fn origin(&self) -> &Origin {
        match self {
            Self::Catalog(uri) => &uri.origin,
            Self::Entry(uri) => &uri.origin,
            Self::DatasetData(uri) => &uri.origin,
            Self::Proxy(uri) => &uri.origin,
        }
    }

    /// Return the parsed `#fragment` of the URI, if any.
    pub fn fragment(&self) -> Option<&Fragment> {
        match self {
            Self::Catalog(_) | Self::Proxy(_) | Self::Entry(_) => None,
            Self::DatasetData(dataset_data_endpoint) => Some(&dataset_data_endpoint.fragment),
        }
    }

    fn partition_id(&self) -> Option<&str> {
        match self {
            Self::Catalog(_) | Self::Proxy(_) | Self::Entry(_) => None,
            Self::DatasetData(dataset_data_uri) => Some(dataset_data_uri.partition_id.as_str()),
        }
    }

    pub fn recording_id(&self) -> Option<StoreId> {
        self.partition_id().map(|partition_id| {
            StoreId::from_string(re_log_types::StoreKind::Recording, partition_id.to_owned())
        })
    }
}

impl std::fmt::Display for RedapUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Catalog(uri) => write!(f, "{uri}",),
            Self::Entry(uri) => write!(f, "{uri}",),
            Self::DatasetData(uri) => write!(f, "{uri}",),
            Self::Proxy(uri) => write!(f, "{uri}",),
        }
    }
}

impl std::str::FromStr for RedapUri {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Hacky, but I don't want to have to memorize ports.
        let default_localhost_port = if value.contains("/proxy") {
            DEFAULT_PROXY_PORT
        } else {
            DEFAULT_REDAP_PORT
        };

        let (origin, http_url) = Origin::replace_and_parse(value, Some(default_localhost_port))?;

        let segments = http_url
            .path_segments()
            .ok_or_else(|| Error::UnexpectedBaseUrl(value.to_owned()))?
            .filter(|s| !s.is_empty()) // handle trailing slashes
            .collect::<Vec<_>>();

        Self::parse_endpoint_with_prefixes(&segments, origin, &http_url)
    }
}

impl RedapUri {
    /// Parse endpoint from path segments, checking from the end to support path prefixes.
    fn parse_endpoint_with_prefixes(
        segments: &[&str],
        origin: Origin,
        http_url: &url::Url,
    ) -> Result<Self, Error> {
        let endpoint_match = Self::match_endpoint(segments)?;

        match endpoint_match {
            EndpointMatch::Proxy => {
                let prefix_segments = Self::extract_proxy_prefix(segments);
                Ok(Self::Proxy(ProxyUri {
                    origin,
                    prefix_segments,
                }))
            }
            EndpointMatch::Catalog => Ok(Self::Catalog(CatalogUri::new(origin))),
            EndpointMatch::Entry { entry_id } => {
                let entry_id =
                    re_log_types::EntryId::from_str(entry_id).map_err(Error::InvalidTuid)?;
                Ok(Self::Entry(EntryUri::new(origin, entry_id)))
            }
            EndpointMatch::Dataset { dataset_id } => {
                let dataset_id = re_tuid::Tuid::from_str(dataset_id).map_err(Error::InvalidTuid)?;
                DatasetDataUri::new(origin, dataset_id, http_url).map(Self::DatasetData)
            }
        }
    }

    /// Extract prefix segments for any endpoint type based on endpoint length.
    fn extract_prefix_segments(segments: &[&str], endpoint_len: usize) -> Option<Vec<String>> {
        if segments.len() <= endpoint_len {
            None
        } else {
            Some(
                segments[..segments.len() - endpoint_len]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            )
        }
    }

    /// Extract prefix segments for proxy endpoints, returning None if no prefix.
    fn extract_proxy_prefix(segments: &[&str]) -> Option<Vec<String>> {
        // Proxy endpoint has 1 segment: "proxy"
        const PROXY_ENDPOINT_LEN: usize = 1;
        Self::extract_prefix_segments(segments, PROXY_ENDPOINT_LEN)
    }

    /// Match endpoint pattern from the end of path segments.
    fn match_endpoint<'a>(segments: &'a [&'a str]) -> Result<EndpointMatch<'a>, Error> {
        match segments {
            // Empty path or explicit catalog endpoint defaults to catalog
            [] | [.., "catalog"] => Ok(EndpointMatch::Catalog),

            // Single segment proxy endpoint
            [.., "proxy"] => Ok(EndpointMatch::Proxy),

            // Two segment endpoints: /entry/{id} and /dataset/{id}
            [.., "entry", entry_id] => Ok(EndpointMatch::Entry { entry_id }),

            // Dataset endpoints: /dataset/{id} or /dataset/{id}/data
            [.., "dataset", dataset_id] | [.., "dataset", dataset_id, "data"] => {
                Ok(EndpointMatch::Dataset { dataset_id })
            }

            // Unknown endpoint
            _ => Err(Error::UnexpectedUri("unknown endpoint".to_owned())),
        }
    }
}

/// Represents a matched endpoint with its parameters.
#[derive(Debug)]
enum EndpointMatch<'a> {
    Proxy,
    Catalog,
    Entry { entry_id: &'a str },
    Dataset { dataset_id: &'a str },
}

// --------------------------------

// Serialize as string:
impl serde::Serialize for RedapUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for RedapUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}

// --------------------------------

#[cfg(test)]
mod tests {
    #![expect(clippy::unnecessary_fallible_conversions)]

    use re_log_types::DataPath;

    use crate::{Fragment, Scheme, TimeRange};

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
        let origin = crate::Origin {
            scheme: Scheme::Rerun,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "https://127.0.0.1:1234");

        let origin = crate::Origin {
            scheme: Scheme::RerunHttp,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "http://127.0.0.1:1234");

        let origin = crate::Origin {
            scheme: Scheme::RerunHttps,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.as_url(), "https://127.0.0.1:1234");
    }

    #[test]
    fn test_entry_url_to_address() {
        let url = "rerun://127.0.0.1:1234/entry/1830B33B45B963E7774455beb91701ae";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::Entry(EntryUri { origin, entry_id }) = address else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            entry_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
    }

    #[test]
    fn test_dataset_data_url_to_address() {
        let url =
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetDataUri {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(time_range, None);
        assert_eq!(fragment, Default::default());
    }

    #[test]
    fn test_dataset_data_url_with_fragment() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid#focus=/some/entity[#42]";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetDataUri {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(time_range, None);
        assert_eq!(
            fragment,
            Fragment {
                focus: Some(DataPath {
                    entity_path: "/some/entity".into(),
                    instance: Some(42.into()),
                    component_descriptor: None,
                }),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_dataset_data_url_time_range_sequence_to_address() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@100..200";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetDataUri {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap()
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(
            time_range,
            Some(TimeRange {
                timeline: re_log_types::Timeline::new_sequence("timeline"),
                min: 100.try_into().unwrap(),
                max: 200.try_into().unwrap(),
            })
        );
        assert_eq!(fragment, Default::default());
    }

    #[test]
    fn test_dataset_data_url_time_range_timepoint_to_address() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=log_time@2022-01-01T00:00:03.123456789Z..2022-01-01T00:00:13.123456789Z";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetDataUri {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap()
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(
            time_range,
            Some(TimeRange {
                timeline: re_log_types::Timeline::new_timestamp("log_time"),
                min: 1_640_995_203_123_456_789.try_into().unwrap(),
                max: 1_640_995_213_123_456_789.try_into().unwrap(),
            })
        );
        assert_eq!(fragment, Default::default());
    }

    #[test]
    fn test_dataset_data_url_time_range_temporal() {
        for url in [
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@1.23s..72s",
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@1230ms..1m12s",
        ] {
            let address: RedapUri = url.parse().unwrap();

            let RedapUri::DatasetData(DatasetDataUri {
                origin,
                dataset_id,
                partition_id,
                time_range,
                fragment,
            }) = address
            else {
                panic!("Expected recording");
            };

            assert_eq!(origin.scheme, Scheme::Rerun);
            assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
            assert_eq!(origin.port, 1234);
            assert_eq!(
                dataset_id,
                "1830B33B45B963E7774455beb91701ae".parse().unwrap()
            );
            assert_eq!(partition_id, "pid");
            assert_eq!(
                time_range,
                Some(TimeRange {
                    timeline: re_log_types::Timeline::new_duration("timeline"),
                    min: re_log_types::TimeInt::from_secs(1.23).try_into().unwrap(),
                    max: re_log_types::TimeInt::from_secs(72.0).try_into().unwrap(),
                })
            );
            assert_eq!(fragment, Default::default());
        }
    }

    #[test]
    fn test_dataset_data_url_missing_partition_id() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data";

        assert!(url.parse::<RedapUri>().is_err());
    }

    #[test]
    fn test_http_catalog_url_to_address() {
        let url = "rerun+http://127.0.0.1:50051/catalog";
        let address: RedapUri = url.parse().unwrap();
        assert!(matches!(
            address,
            RedapUri::Catalog(CatalogUri {
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
        let address: RedapUri = url.parse().unwrap();

        assert!(matches!(
            address,
            RedapUri::Catalog(CatalogUri {
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
        let address: RedapUri = url.parse().unwrap();

        assert_eq!(
            address,
            RedapUri::Catalog(CatalogUri {
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
        let address: Result<RedapUri, _> = url.parse();

        assert!(matches!(
            address.unwrap_err(),
            super::Error::InvalidScheme { .. }
        ));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapUri, _> = url.parse();

        // The new logic returns "unknown endpoint" for unrecognized paths
        assert!(matches!(
            address.unwrap_err(),
            super::Error::UnexpectedUri(unknown) if unknown == "unknown endpoint"
        ));
    }

    #[test]
    fn test_proxy_endpoint() {
        let url = "rerun://localhost:51234/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            origin: Origin {
                scheme: Scheme::Rerun,
                host: url::Host::Domain("localhost".to_owned()),
                port: 51234,
            },
            prefix_segments: None,
        });

        assert_eq!(address.unwrap(), expected);

        let url = "rerun://localhost:51234/proxy/";
        let address: Result<RedapUri, _> = url.parse();

        assert_eq!(address.unwrap(), expected);

        // Test proxy endpoint with single path prefix
        let url = "rerun+http://13.31.13.31/rerun/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            origin: Origin {
                scheme: Scheme::RerunHttp,
                host: url::Host::Ipv4("13.31.13.31".parse().unwrap()),
                port: 80,
            },
            prefix_segments: Some(vec!["rerun".to_owned()]),
        });

        assert_eq!(address.unwrap(), expected);

        // Test proxy endpoint with multi-segment path prefix
        let url = "rerun+http://13.31.13.31/cell/vscode/rerun/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            origin: Origin {
                scheme: Scheme::RerunHttp,
                host: url::Host::Ipv4("13.31.13.31".parse().unwrap()),
                port: 80,
            },
            prefix_segments: Some(vec![
                "cell".to_owned(),
                "vscode".to_owned(),
                "rerun".to_owned(),
            ]),
        });

        assert_eq!(address.unwrap(), expected);
    }

    #[test]
    fn test_parsing() {
        let test_cases = [
            (
                "rerun://localhost/catalog",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::Rerun,
                        host: url::Host::Domain("localhost".to_owned()),
                        port: DEFAULT_REDAP_PORT,
                    },
                }),
            ),
            (
                "localhost",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Domain("localhost".to_owned()),
                        port: DEFAULT_REDAP_PORT,
                    },
                }),
            ),
            (
                "localhost/proxy",
                RedapUri::Proxy(ProxyUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Domain("localhost".to_owned()),
                        port: DEFAULT_PROXY_PORT,
                    },
                    prefix_segments: None,
                }),
            ),
            (
                "127.0.0.1/proxy",
                RedapUri::Proxy(ProxyUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Ipv4(Ipv4Addr::new(127, 0, 0, 1)),
                        port: DEFAULT_PROXY_PORT,
                    },
                    prefix_segments: None,
                }),
            ),
            (
                "rerun+http://example.com",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Domain("example.com".to_owned()),
                        port: 80,
                    },
                }),
            ),
            (
                "rerun+https://example.com",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttps,
                        host: url::Host::Domain("example.com".to_owned()),
                        port: 443,
                    },
                }),
            ),
            (
                "rerun://example.com",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::Rerun,
                        host: url::Host::Domain("example.com".to_owned()),
                        port: 443,
                    },
                }),
            ),
            (
                "rerun://example.com:420/catalog",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::Rerun,
                        host: url::Host::Domain("example.com".to_owned()),
                        port: 420,
                    },
                }),
            ),
        ];

        for (url, expected) in test_cases {
            assert_eq!(
                url.parse::<RedapUri>()
                    .unwrap_or_else(|err| panic!("Failed to parse url {url:}: {err}")),
                expected,
                "Url: {url:?}"
            );
        }
    }

    #[test]
    fn test_catalog_default() {
        let url = "rerun://localhost:51234";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Catalog(CatalogUri {
            origin: Origin {
                scheme: Scheme::Rerun,
                host: url::Host::Domain("localhost".to_owned()),
                port: 51234,
            },
        });

        assert_eq!(address.unwrap(), expected);

        let url = "rerun://localhost:51234/";
        let address: Result<RedapUri, _> = url.parse();

        assert_eq!(address.unwrap(), expected);
    }

    #[test]
    fn test_custom_port() {
        let url = "rerun://localhost:123";

        let expected = RedapUri::Catalog(CatalogUri {
            origin: Origin {
                scheme: Scheme::Rerun,
                host: url::Host::Domain("localhost".to_owned()),
                port: 123,
            },
        });

        assert_eq!(url.parse::<RedapUri>().unwrap(), expected);
    }

    #[test]
    fn test_invalid_endpoints_rejected() {
        // Test that unknown endpoints are rejected to reserve them for future use
        let invalid_urls = [
            "rerun://localhost/unknown",
            "rerun://localhost/invalid/endpoint",
            "rerun://localhost/prefix/unknown",
            "rerun://localhost/some/random/path",
        ];

        for url in invalid_urls {
            let result: Result<RedapUri, _> = url.parse();
            assert!(
                result.is_err(),
                "URL {url} should be rejected but was accepted"
            );
        }
    }
    #[test]
    fn test_path_prefixes() {
        // Test that all endpoint types support path prefixes (regression test for #10373)

        // Test proxy endpoints specifically (the original issue)
        let proxy_test_cases = [
            ("rerun://localhost/proxy", None),
            ("rerun+http://13.31.13.31/rerun/proxy", Some(vec!["rerun"])),
            (
                "rerun+http://host/cell/vscode/rerun/proxy",
                Some(vec!["cell", "vscode", "rerun"]),
            ),
            (
                "rerun://localhost/a/b/c/d/e/proxy",
                Some(vec!["a", "b", "c", "d", "e"]),
            ),
        ];

        for (url, expected_prefix) in proxy_test_cases {
            let result: RedapUri = url
                .parse()
                .unwrap_or_else(|err| panic!("Failed to parse proxy URL {url}: {err}"));

            if let RedapUri::Proxy(proxy_uri) = result {
                let expected_segments =
                    expected_prefix.map(|p| p.into_iter().map(String::from).collect());
                assert_eq!(
                    proxy_uri.prefix_segments, expected_segments,
                    "Proxy URL {url} parsed with wrong prefix segments"
                );
            } else {
                panic!("URL {url} should parse as Proxy but got different type");
            }
        }

        // Test that other endpoint types also accept prefixes (they just don't store them)
        let other_endpoint_test_cases = [
            ("rerun://localhost/catalog", "Catalog"),
            ("rerun://localhost/prefix/catalog", "Catalog"),
            (
                "rerun://localhost/a/b/entry/1830B33B45B963E7774455beb91701ae",
                "Entry",
            ),
            (
                "rerun://localhost/x/y/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
                "DatasetData",
            ),
        ];

        for (url, expected_type) in other_endpoint_test_cases {
            let result: RedapUri = url
                .parse()
                .unwrap_or_else(|err| panic!("Failed to parse URL {url}: {err}"));

            let actual_type = match result {
                RedapUri::Proxy(_) => "Proxy",
                RedapUri::Catalog(_) => "Catalog",
                RedapUri::Entry(_) => "Entry",
                RedapUri::DatasetData(_) => "DatasetData",
            };

            assert_eq!(actual_type, expected_type, "URL {url} parsed as wrong type");
        }
    }

    #[test]
    fn test_proxy_uri_round_trip() {
        // Test round-trip parsing for proxy URIs with various path prefix configurations
        // This addresses the specific issue #10373 and ensures path prefixes are preserved correctly

        let test_cases = [
            // No prefix
            ("rerun://localhost/proxy", None),
            ("rerun+http://127.0.0.1:9876/proxy", None),
            // Single prefix segment
            ("rerun+http://13.31.13.31/rerun/proxy", Some(vec!["rerun"])),
            ("rerun://localhost/prefix/proxy", Some(vec!["prefix"])),
            // Multiple prefix segments
            (
                "rerun+http://host/cell/vscode/rerun/proxy",
                Some(vec!["cell", "vscode", "rerun"]),
            ),
            (
                "rerun://localhost/a/b/c/d/e/proxy",
                Some(vec!["a", "b", "c", "d", "e"]),
            ),
            // Edge cases
            (
                "rerun+https://example.com:8080/single/proxy",
                Some(vec!["single"]),
            ),
        ];

        for (original_url, expected_prefix) in test_cases {
            // Parse the original URL
            let parsed_uri: RedapUri = original_url
                .parse()
                .unwrap_or_else(|err| panic!("Failed to parse URL {original_url}: {err}"));

            // Verify it's a proxy URI with correct prefix
            if let RedapUri::Proxy(proxy_uri) = &parsed_uri {
                let expected_segments =
                    expected_prefix.map(|p| p.into_iter().map(String::from).collect());
                assert_eq!(
                    proxy_uri.prefix_segments, expected_segments,
                    "URL {original_url} parsed with wrong prefix segments"
                );
            } else {
                panic!("URL {original_url} should parse as Proxy but got different type");
            }

            // Test round-trip: convert back to string and parse again
            let round_trip_url = parsed_uri.to_string();
            let round_trip_parsed: RedapUri = round_trip_url.parse().unwrap_or_else(|err| {
                panic!("Round-trip parsing failed for {round_trip_url}: {err}")
            });

            // Verify the round-trip result matches the original
            if let (RedapUri::Proxy(original), RedapUri::Proxy(round_trip)) =
                (&parsed_uri, &round_trip_parsed)
            {
                assert_eq!(
                    original.prefix_segments, round_trip.prefix_segments,
                    "Round-trip failed: prefix segments differ for {original_url}"
                );
                assert_eq!(
                    original.origin, round_trip.origin,
                    "Round-trip failed: origin differs for {original_url}"
                );
            } else {
                panic!("Round-trip parsing changed URI type for {original_url}");
            }
        }
    }

    #[test]
    fn resolved_endpoints_with_prefix() {
        let cases = [
            // HTTP - Only proxy URIs currently support path prefixes
            (
                "rerun+http://localhost/a/b/proxy",
                "http://localhost:9876/a/b",
            ),
            // Other endpoint types currently only return origin URLs (no path prefix support)
            (
                "rerun+http://localhost/a/b/catalog",
                "http://localhost:51234", // Note: no /a/b path prefix
            ),
            (
                "rerun+http://localhost/a/b/entry/1830B33B45B963E7774455beb91701ae",
                "http://localhost:51234", // Note: no /a/b path prefix
            ),
            (
                "rerun+http://localhost/a/b/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
                "http://localhost:51234", // Note: no /a/b path prefix
            ),
            // HTTPS - Only proxy URIs currently support path prefixes
            (
                "rerun://example.com/foo/bar/proxy",
                "https://example.com:443/foo/bar",
            ),
            // Other endpoint types currently only return origin URLs (no path prefix support)  
            (
                "rerun://example.com/foo/bar/catalog",
                "https://example.com:443", // Note: no /foo/bar path prefix
            ),
            (
                "rerun://example.com/foo/bar/entry/1830B33B45B963E7774455beb91701ae",
                "https://example.com:443", // Note: no /foo/bar path prefix
            ),
            (
                "rerun://example.com/foo/bar/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
                "https://example.com:443", // Note: no /foo/bar path prefix
            ),
        ];

        for (url, expected) in cases {
            let result: RedapUri = url.parse().expect("failed to parse URI");
            
            let actual_url = match &result {
                RedapUri::Proxy(proxy_uri) => proxy_uri.endpoint_url(),
                _ => result.origin().as_url(),
            };
            
            assert_eq!(
                expected,
                actual_url,
                "failed to resolve {url}"
            );
        }
    }
}
