use re_log_types::StoreId;

use crate::{
    CatalogUri, DEFAULT_PROXY_PORT, DEFAULT_REDAP_PORT, DatasetPartitionUri, EndpointAddr,
    EntryUri, Error, Fragment, Origin, ProxyUri,
};

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RedapUri {
    /// `/catalog` - also the default if there is no /endpoint
    Catalog(CatalogUri),

    /// `/entry`
    Entry(EntryUri),

    /// `/dataset`
    DatasetData(DatasetPartitionUri),

    /// We use the `/proxy` endpoint to access another _local_ viewer.
    Proxy(ProxyUri),
}

impl RedapUri {
    pub fn origin(&self) -> &Origin {
        match self {
            Self::Catalog(uri) => &uri.endpoint_addr.origin,
            Self::Entry(uri) => &uri.endpoint_addr.origin,
            Self::DatasetData(uri) => &uri.endpoint_addr.origin,
            Self::Proxy(uri) => &uri.endpoint_addr.origin,
        }
    }

    pub fn endpoint_addr(&self) -> &EndpointAddr {
        match self {
            Self::Catalog(uri) => &uri.endpoint_addr,
            Self::Entry(uri) => &uri.endpoint_addr,
            Self::DatasetData(uri) => &uri.endpoint_addr,
            Self::Proxy(uri) => &uri.endpoint_addr,
        }
    }

    /// Return the parsed `#fragment` of the URI, if any.
    pub fn fragment(&self) -> Option<&Fragment> {
        match self {
            Self::Catalog(_) | Self::Proxy(_) | Self::Entry(_) => None,
            Self::DatasetData(dataset_data_endpoint) => Some(&dataset_data_endpoint.fragment),
        }
    }

    pub fn store_id(&self) -> Option<StoreId> {
        match self {
            Self::Catalog(_) | Self::Entry(_) | Self::Proxy(_) => None,
            Self::DatasetData(dataset_data_uri) => Some(dataset_data_uri.store_id()),
        }
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

        let create_endpoint_addr = |prefix_segments: &[&str]| {
            let mut addr = EndpointAddr::new(origin.clone());
            if !prefix_segments.is_empty() {
                let prefix = format!("/{}", prefix_segments.join("/"));
                addr = addr.with_path_prefix(prefix);
            }
            addr
        };

        // 1. Terminal endpoints (endpoints that must be at the end of the path)
        match segments.as_slice() {
            [prefix_segments @ .., "proxy"] => {
                let endpoint_addr = create_endpoint_addr(prefix_segments);
                return Ok(Self::Proxy(ProxyUri::new(endpoint_addr)));
            }
            [prefix_segments @ .., "catalog"] => {
                let endpoint_addr = create_endpoint_addr(prefix_segments);
                return Ok(Self::Catalog(CatalogUri::new(endpoint_addr)));
            }
            [] => {
                let endpoint_addr = create_endpoint_addr(&[]);
                return Ok(Self::Catalog(CatalogUri::new(endpoint_addr)));
            }
            _ => {}
        }

        // 2. Middle endpoints (endpoints that are followed by arguments)
        // We look for the last occurrence of a known endpoint keyword.
        let found_endpoint = segments
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, s)| match *s {
                "entry" if segments.len() > i + 1 => Some((i, "entry")),
                "dataset" if segments.len() > i + 1 => Some((i, "dataset")),
                _ => None,
            });

        if let Some((i, kind)) = found_endpoint {
            let prefix_segments = &segments[..i];
            let endpoint_addr = create_endpoint_addr(prefix_segments);

            match kind {
                "entry" => {
                    let entry_id_str = &segments[i + 1];
                    let entry_id =
                        std::str::FromStr::from_str(entry_id_str).map_err(Error::InvalidTuid)?;
                    Ok(Self::Entry(EntryUri::new(endpoint_addr, entry_id)))
                }
                "dataset" => {
                    let dataset_id_str = &segments[i + 1];
                    let dataset_id =
                        re_tuid::Tuid::from_str(dataset_id_str).map_err(Error::InvalidTuid)?;
                    DatasetPartitionUri::new(endpoint_addr, dataset_id, &http_url)
                        .map(Self::DatasetData)
                }
                _ => unreachable!(),
            }
        } else {
            // No known endpoint found
            Err(Error::UnexpectedUri(value.to_owned()))
        }
    }
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
    use re_log_types::DataPath;

    use crate::{Fragment, Scheme, TimeSelection};

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

        let RedapUri::Entry(EntryUri {
            endpoint_addr,
            entry_id,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
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

        let RedapUri::DatasetData(DatasetPartitionUri {
            endpoint_addr,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
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
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid#selection=/some/entity[#42]";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetPartitionUri {
            endpoint_addr,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(time_range, None);
        assert_eq!(
            fragment,
            Fragment {
                selection: Some(DataPath {
                    entity_path: "/some/entity".into(),
                    instance: Some(42.into()),
                    component: None,
                }),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_dataset_data_url_with_broken_fragment() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid#focus=/some/entity[#42]";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetPartitionUri {
            endpoint_addr,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(time_range, None);
        assert_eq!(fragment, Fragment::default());
    }

    #[test]
    fn test_dataset_data_url_time_range_sequence_to_address() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@100..200";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetPartitionUri {
            endpoint_addr,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap()
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(
            time_range,
            Some(TimeSelection {
                timeline: re_log_types::Timeline::new_sequence("timeline"),
                range: re_log_types::AbsoluteTimeRange::new(100, 200),
            })
        );
        assert_eq!(fragment, Default::default());
    }

    #[test]
    fn test_dataset_data_url_time_range_timepoint_to_address() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=log_time@2022-01-01T00:00:03.123456789Z..2022-01-01T00:00:13.123456789Z";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetPartitionUri {
            endpoint_addr,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
        assert_eq!(
            endpoint_addr.origin.host,
            url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(endpoint_addr.origin.port, 1234);
        assert_eq!(endpoint_addr.path_prefix, None);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap()
        );
        assert_eq!(partition_id, "pid");
        assert_eq!(
            time_range,
            Some(TimeSelection {
                timeline: re_log_types::Timeline::new_timestamp("log_time"),
                range: re_log_types::AbsoluteTimeRange::new(
                    1_640_995_203_123_456_789,
                    1_640_995_213_123_456_789,
                ),
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

            let RedapUri::DatasetData(DatasetPartitionUri {
                endpoint_addr,
                dataset_id,
                partition_id,
                time_range,
                fragment,
            }) = address
            else {
                panic!("Expected recording");
            };

            assert_eq!(endpoint_addr.origin.scheme, Scheme::Rerun);
            assert_eq!(
                endpoint_addr.origin.host,
                url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST)
            );
            assert_eq!(endpoint_addr.origin.port, 1234);
            assert_eq!(endpoint_addr.path_prefix, None);
            assert_eq!(
                dataset_id,
                "1830B33B45B963E7774455beb91701ae".parse().unwrap()
            );
            assert_eq!(partition_id, "pid");
            assert_eq!(
                time_range,
                Some(TimeSelection {
                    timeline: re_log_types::Timeline::new_duration("timeline"),
                    range: re_log_types::AbsoluteTimeRange::new(
                        re_log_types::TimeInt::from_secs(1.23),
                        re_log_types::TimeInt::from_secs(72.0),
                    ),
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
        assert_eq!(
            address,
            RedapUri::Catalog(CatalogUri {
                endpoint_addr: EndpointAddr {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                        port: 50051
                    },
                    path_prefix: None,
                }
            })
        );
    }

    #[test]
    fn test_https_catalog_url_to_address() {
        let url = "rerun+https://127.0.0.1:50051/catalog";
        let address: RedapUri = url.parse().unwrap();

        assert_eq!(
            address,
            RedapUri::Catalog(CatalogUri {
                endpoint_addr: EndpointAddr {
                    origin: Origin {
                        scheme: Scheme::RerunHttps,
                        host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                        port: 50051
                    },
                    path_prefix: None,
                }
            })
        );
    }

    #[test]
    fn test_localhost_url() {
        let url = "rerun+http://localhost:51234/catalog";
        let address: RedapUri = url.parse().unwrap();

        assert_eq!(
            address,
            RedapUri::Catalog(CatalogUri {
                endpoint_addr: EndpointAddr {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::<String>::Domain("localhost".to_owned()),
                        port: 51234
                    },
                    path_prefix: None,
                }
            })
        );
    }

    #[test]
    fn test_invalid_url() {
        let url = "http://wrong-scheme:1234/recording/12345";
        let address: Result<RedapUri, _> = url.parse();

        assert!(matches!(address.unwrap_err(), super::Error::InvalidScheme));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapUri, _> = url.parse();

        assert!(matches!(
            address.unwrap_err(),
            super::Error::UnexpectedUri(unknown) if unknown == url
        ));
    }

    #[test]
    fn test_proxy_endpoint() {
        let url = "rerun://localhost:51234/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            endpoint_addr: EndpointAddr {
                origin: Origin {
                    scheme: Scheme::Rerun,
                    host: url::Host::Domain("localhost".to_owned()),
                    port: 51234,
                },
                path_prefix: None,
            },
        });

        assert_eq!(address.unwrap(), expected);

        let url = "rerun://localhost:51234/proxy/";
        let address: Result<RedapUri, _> = url.parse();

        assert_eq!(address.unwrap(), expected);
    }

    #[test]
    fn test_parsing() {
        let test_cases = [
            (
                "rerun://localhost/catalog",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: DEFAULT_REDAP_PORT,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "localhost",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttp,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: DEFAULT_REDAP_PORT,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "localhost/proxy",
                RedapUri::Proxy(ProxyUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttp,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: DEFAULT_PROXY_PORT,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "127.0.0.1/proxy",
                RedapUri::Proxy(ProxyUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttp,
                            host: url::Host::Ipv4(Ipv4Addr::new(127, 0, 0, 1)),
                            port: DEFAULT_PROXY_PORT,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "rerun+http://example.com",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttp,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 80,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "rerun+https://example.com",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttps,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 443,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "rerun://example.com",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 443,
                        },
                        path_prefix: None,
                    },
                }),
            ),
            (
                "rerun://example.com:420/catalog",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 420,
                        },
                        path_prefix: None,
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
            endpoint_addr: EndpointAddr {
                origin: Origin {
                    scheme: Scheme::Rerun,
                    host: url::Host::Domain("localhost".to_owned()),
                    port: 51234,
                },
                path_prefix: None,
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
            endpoint_addr: EndpointAddr {
                origin: Origin {
                    scheme: Scheme::Rerun,
                    host: url::Host::Domain("localhost".to_owned()),
                    port: 123,
                },
                path_prefix: None,
            },
        });

        assert_eq!(url.parse::<RedapUri>().unwrap(), expected);
    }

    #[test]
    fn test_parsing_with_subpaths() {
        let test_cases = [
            (
                "rerun+http://example.com/sub/path/proxy",
                RedapUri::Proxy(ProxyUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::RerunHttp,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 80,
                        },
                        path_prefix: Some("/sub/path".to_owned()),
                    },
                }),
            ),
            (
                "rerun://localhost:1234/entry/1830B33B45B963E7774455beb91701ae/extra/junk",
                RedapUri::Entry(EntryUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: None,
                    },
                    entry_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                }),
            ),
            (
                "rerun://localhost:1234/dataset/1830B33B45B963E7774455beb91701ae/extra/junk?partition_id=pid",
                RedapUri::DatasetData(DatasetPartitionUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: None,
                    },
                    dataset_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                    partition_id: "pid".to_owned(),
                    time_range: None,
                    fragment: Fragment::default(),
                }),
            ),
            (
                "rerun://example.com/sub/path/entry/1830B33B45B963E7774455beb91701ae",
                RedapUri::Entry(EntryUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("example.com".to_owned()),
                            port: 443,
                        },
                        path_prefix: Some("/sub/path".to_owned()),
                    },
                    entry_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
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
    fn test_parsing_messy_slashes() {
        let test_cases = [
            (
                "rerun://localhost:1234//sub//path//catalog/",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: Some("/sub/path".to_owned()),
                    },
                }),
            ),
            (
                "rerun://localhost:1234/sub/path/catalog//",
                RedapUri::Catalog(CatalogUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: Some("/sub/path".to_owned()),
                    },
                }),
            ),
            (
                "rerun://localhost:1234//entry//1830B33B45B963E7774455beb91701ae",
                RedapUri::Entry(EntryUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: None,
                    },
                    entry_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                }),
            ),
            (
                "rerun://localhost:1234/sub//path//dataset/1830B33B45B963E7774455beb91701ae//data?partition_id=pid",
                RedapUri::DatasetData(DatasetPartitionUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: Some("/sub/path".to_owned()),
                    },
                    dataset_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                    partition_id: "pid".to_owned(),
                    time_range: None,
                    fragment: Fragment::default(),
                }),
            ),
            (
                "rerun://localhost:1234//entry//1830B33B45B963E7774455beb91701ae//extra//junk",
                RedapUri::Entry(EntryUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: None,
                    },
                    entry_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                }),
            ),
            (
                "rerun://localhost:1234//dataset//1830B33B45B963E7774455beb91701ae//extra//junk//data?partition_id=pid",
                RedapUri::DatasetData(DatasetPartitionUri {
                    endpoint_addr: EndpointAddr {
                        origin: Origin {
                            scheme: Scheme::Rerun,
                            host: url::Host::Domain("localhost".to_owned()),
                            port: 1234,
                        },
                        path_prefix: None,
                    },
                    dataset_id: "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
                    partition_id: "pid".to_owned(),
                    time_range: None,
                    fragment: Fragment::default(),
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
}
