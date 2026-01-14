use re_log_types::StoreId;

use crate::{
    CatalogUri, DEFAULT_PROXY_PORT, DEFAULT_REDAP_PORT, DatasetSegmentUri, EntryUri, Error,
    Fragment, Origin, ProxyUri,
};

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RedapUri {
    /// `/catalog` - also the default if there is no /endpoint
    Catalog(CatalogUri),

    /// `/entry`
    Entry(EntryUri),

    /// `/dataset`
    DatasetData(DatasetSegmentUri),

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

        // :warning: We limit the amount of segments, which might need to be
        // adjusted when adding additional resources.
        let segments = http_url
            .path_segments()
            .ok_or_else(|| Error::UnexpectedBaseUrl(value.to_owned()))?
            .take(2)
            .filter(|s| !s.is_empty()) // handle trailing slashes
            .collect::<Vec<_>>();

        match segments.as_slice() {
            ["proxy"] => Ok(Self::Proxy(ProxyUri::new(origin))),

            ["catalog"] | [] => Ok(Self::Catalog(CatalogUri::new(origin))),

            ["entry", entry_id] => {
                let entry_id =
                    re_log_types::EntryId::from_str(entry_id).map_err(Error::InvalidTuid)?;
                Ok(Self::Entry(EntryUri::new(origin, entry_id)))
            }

            ["dataset", dataset_id] => {
                let dataset_id = re_tuid::Tuid::from_str(dataset_id).map_err(Error::InvalidTuid)?;

                DatasetSegmentUri::new(origin, dataset_id, &http_url).map(Self::DatasetData)
            }
            [unknown, ..] => Err(Error::UnexpectedUri(format!("{unknown}/"))),
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
    use core::net::Ipv4Addr;

    use re_log_types::DataPath;

    use super::*;
    use crate::{DatasetSegmentUri, Fragment, Scheme};

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
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetSegmentUri {
            origin,
            dataset_id,
            segment_id,
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
        assert_eq!(segment_id, "sid");
        assert_eq!(fragment, Default::default());
    }

    /// Test that `partition_id` still works for backward compatibility.
    #[test]
    fn test_dataset_data_url_legacy_partition_id() {
        let url =
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetSegmentUri { segment_id, .. }) = address else {
            panic!("Expected recording");
        };

        // Legacy `partition_id` is parsed into `segment_id`.
        assert_eq!(segment_id, "pid");
    }

    /// Test that `segment_id` and `partition_id` together do not work.
    #[test]
    fn test_dataset_data_url_ambiguous_segment_id_partition_id() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&segment_id=sid";
        let address: Result<RedapUri, _> = url.parse();

        assert_eq!(address, Err(Error::AmbiguousSegmentId));
    }

    #[test]
    fn test_dataset_data_url_with_fragment() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid#selection=/some/entity[#42]";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetSegmentUri {
            origin,
            dataset_id,
            segment_id,
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
        assert_eq!(segment_id, "sid");
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
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid#focus=/some/entity[#42]";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::DatasetData(DatasetSegmentUri {
            origin,
            dataset_id,
            segment_id,
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
        assert_eq!(segment_id, "sid");
        assert_eq!(fragment, Fragment::default());
    }

    #[test]
    fn test_dataset_data_url_missing_segment_id() {
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

        assert!(matches!(address.unwrap_err(), super::Error::InvalidScheme));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapUri, _> = url.parse();

        assert!(matches!(
            address.unwrap_err(),
            super::Error::UnexpectedUri(unknown) if &unknown == "redap/"
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
                }),
            ),
            (
                "127.0.0.1/proxy",
                RedapUri::Proxy(ProxyUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttp,
                        host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                        port: DEFAULT_PROXY_PORT,
                    },
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
}
