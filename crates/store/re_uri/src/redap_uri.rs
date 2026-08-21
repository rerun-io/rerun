use re_log_types::StoreId;

use crate::{
    CatalogUri, DEFAULT_PROXY_PORT, DEFAULT_REDAP_PORT, DatasetUri, EntryUri, Error, FolderUri,
    Fragment, Origin, ProxyUri,
};

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RedapUri {
    /// `/catalog` - also the default if there is no /endpoint
    Catalog(CatalogUri),

    /// `/entry/<entry_id>`
    Entry(EntryUri),

    /// `/folder/<dotted.path>` — a dataset-name prefix grouping.
    Folder(FolderUri),

    /// `/dataset/<dataset_id>[/<resource>]`
    Dataset(DatasetUri),

    /// We use the `/proxy` endpoint to access another _local_ viewer.
    Proxy(ProxyUri),
}

impl RedapUri {
    pub fn origin(&self) -> &Origin {
        match self {
            Self::Catalog(uri) => &uri.origin,
            Self::Entry(uri) => &uri.origin,
            Self::Folder(uri) => &uri.origin,
            Self::Dataset(uri) => &uri.origin,
            Self::Proxy(uri) => &uri.origin,
        }
    }

    /// Return the parsed `#fragment` of the URI, if any.
    pub fn fragment(&self) -> Option<&Fragment> {
        match self {
            Self::Catalog(_) | Self::Proxy(_) | Self::Entry(_) | Self::Folder(_) => None,
            Self::Dataset(dataset_uri) => Some(&dataset_uri.fragment),
        }
    }

    pub fn store_id(&self) -> Option<StoreId> {
        match self {
            Self::Catalog(_) | Self::Entry(_) | Self::Folder(_) | Self::Proxy(_) => None,
            Self::Dataset(dataset_uri) => dataset_uri.store_id(),
        }
    }
}

impl std::fmt::Display for RedapUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Catalog(uri) => write!(f, "{uri}"),
            Self::Entry(uri) => write!(f, "{uri}"),
            Self::Folder(uri) => write!(f, "{uri}"),
            Self::Dataset(uri) => write!(f, "{uri}"),
            Self::Proxy(uri) => write!(f, "{uri}"),
        }
    }
}

impl std::str::FromStr for RedapUri {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // If someone manually visits `https://rerun.io/viewer?url=rerun+https://…` then
        // that `+` will be turned into a space. So let's gracefully handle that here:
        let input = &input
            .replace("rerun http", "rerun+http")
            .replace("rerun https", "rerun+https");

        // Hacky, but I don't want to have to memorize ports.
        let default_localhost_port = if input.contains("/proxy") {
            DEFAULT_PROXY_PORT
        } else {
            DEFAULT_REDAP_PORT
        };

        let (origin, http_url) = Origin::replace_and_parse(input, Some(default_localhost_port))?;

        // :warning: We limit the amount of segments, which might need to be
        // adjusted when adding additional resources.
        let segments = http_url
            .path_segments()
            .ok_or_else(|| Error::UnexpectedBaseUrl(input.to_owned()))?
            .take(3)
            .filter(|s| !s.is_empty()) // handle trailing slashes
            .collect::<Vec<_>>();

        match segments.as_slice() {
            ["proxy"] => Ok(Self::Proxy(ProxyUri::new(origin))),

            ["catalog"] | [] => Ok(Self::Catalog(CatalogUri::new(origin))),

            ["entry", entry_id, ..] => {
                let entry_id =
                    re_log_types::EntryId::from_str(entry_id).map_err(Error::InvalidTuid)?;

                Ok(Self::Entry(EntryUri::new(origin, entry_id)))
            }

            ["folder", path, ..] => {
                let decoded = percent_encoding::percent_decode_str(path)
                    .decode_utf8()
                    .map_err(|_err| Error::UnexpectedUri(format!("folder/{path}")))?;
                if decoded.is_empty() {
                    return Err(Error::UnexpectedUri("folder/".to_owned()));
                }
                Ok(Self::Folder(FolderUri::new(origin, decoded.into_owned())))
            }

            ["dataset", dataset_id, rest @ ..] => {
                let dataset_id = re_tuid::Tuid::from_str(dataset_id).map_err(Error::InvalidTuid)?;

                // Fall back to the default resource if the url names one we don't know,
                // which may happen for urls from prior/newer versions.
                let resource = rest
                    .first()
                    .and_then(|resource| crate::DatasetResource::from_str(resource).ok())
                    .unwrap_or_default();

                DatasetUri::new(origin, dataset_id, resource, &http_url).map(Self::Dataset)
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
    use std::assert_matches;

    use re_log_types::DataPath;
    use re_types_core::SegmentId;

    use super::*;
    use crate::{DatasetResource, DatasetUri, Fragment, Scheme};

    #[test]
    fn scheme_conversion() {
        assert_eq!(Scheme::RerunHttps.as_http_scheme(), "https");
        assert_eq!(Scheme::RerunHttp.as_http_scheme(), "http");
    }

    #[test]
    fn origin_conversion() {
        let origin = crate::Origin {
            scheme: Scheme::RerunHttps,
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

        assert_eq!(origin.scheme, Scheme::RerunHttps);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            entry_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
    }

    /// An entry url with a trailing path segment still parses, so that urls from prior versions
    /// keep opening the entry.
    #[test]
    fn test_entry_url_trailing_path() {
        let url = "rerun://127.0.0.1:1234/entry/1830B33B45B963E7774455beb91701ae/whatever";

        let RedapUri::Entry(entry) = url.parse::<RedapUri>().unwrap() else {
            panic!("Expected an entry");
        };
        assert_eq!(
            entry.entry_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
    }

    #[test]
    fn test_dataset_data_url_to_address() {
        let url =
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::Dataset(DatasetUri {
            origin,
            dataset_id,
            resource,
            segment_id,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(resource, DatasetResource::Segments);
        assert_eq!(origin.scheme, Scheme::RerunHttps);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(segment_id.as_ref().map(SegmentId::as_str), Some("sid"));
        assert_eq!(fragment, Default::default());
    }

    /// The resource a dataset url points at is a trailing path segment, and survives being
    /// formatted back into a url. The default resource is left out of the url entirely.
    #[test]
    fn test_dataset_url_resource_roundtrip() {
        for (url, expected_resource) in [
            (
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/assets?segment_id=robot_mesh",
                DatasetResource::Assets,
            ),
            (
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae?segment_id=robot_mesh",
                DatasetResource::Segments,
            ),
            (
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/assets",
                DatasetResource::Assets,
            ),
            (
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae",
                DatasetResource::Segments,
            ),
        ] {
            let uri: RedapUri = url.parse().unwrap();

            let RedapUri::Dataset(dataset) = &uri else {
                panic!("Expected a dataset");
            };
            assert_eq!(dataset.resource, expected_resource);

            assert_eq!(uri.to_string(), url);
        }
    }

    /// A resource we don't know falls back to the default one, so that urls from other versions
    /// still open the dataset.
    #[test]
    fn test_dataset_url_unknown_resource() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/whatever?segment_id=sid";

        let RedapUri::Dataset(dataset) = url.parse::<RedapUri>().unwrap() else {
            panic!("Expected a dataset");
        };
        assert_eq!(dataset.resource, DatasetResource::default());
    }

    /// Test that `partition_id` still works for backward compatibility.
    #[test]
    fn test_dataset_data_url_legacy_partition_id() {
        let url =
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid";
        let address: RedapUri = url.parse().unwrap();

        let RedapUri::Dataset(DatasetUri { segment_id, .. }) = address else {
            panic!("Expected recording");
        };

        // Legacy `partition_id` is parsed into `segment_id`.
        assert_eq!(segment_id.as_ref().map(SegmentId::as_str), Some("pid"));
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

        let RedapUri::Dataset(DatasetUri {
            origin,
            dataset_id,
            resource,
            segment_id,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(resource, DatasetResource::Segments);
        assert_eq!(origin.scheme, Scheme::RerunHttps);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(segment_id.as_ref().map(SegmentId::as_str), Some("sid"));
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

        let RedapUri::Dataset(DatasetUri {
            origin,
            dataset_id,
            resource,
            segment_id,
            fragment,
        }) = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(resource, DatasetResource::Segments);
        assert_eq!(origin.scheme, Scheme::RerunHttps);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(
            dataset_id,
            "1830B33B45B963E7774455beb91701ae".parse().unwrap(),
        );
        assert_eq!(segment_id.as_ref().map(SegmentId::as_str), Some("sid"));
        assert_eq!(fragment, Fragment::default());
    }

    /// A dataset url without a `segment_id` points at the dataset itself rather than at a segment
    /// to load.
    #[test]
    fn test_dataset_url_without_segment_id() {
        let url = "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae";

        let RedapUri::Dataset(dataset) = url.parse::<RedapUri>().unwrap() else {
            panic!("Expected a dataset");
        };
        assert_eq!(dataset.segment_id, None);
        assert_eq!(dataset.store_id(), None);
    }

    #[test]
    fn test_http_catalog_url_to_address() {
        let url = "rerun+http://127.0.0.1:50051/catalog";
        let address: RedapUri = url.parse().unwrap();
        assert_matches!(
            address,
            RedapUri::Catalog(CatalogUri {
                origin: Origin {
                    scheme: Scheme::RerunHttp,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
                },
            })
        );
    }

    #[test]
    fn test_https_catalog_url_to_address() {
        let url = "rerun+https://127.0.0.1:50051/catalog";
        let address: RedapUri = url.parse().unwrap();

        assert_matches!(
            address,
            RedapUri::Catalog(CatalogUri {
                origin: Origin {
                    scheme: Scheme::RerunHttps,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
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

        assert_matches!(address.unwrap_err(), super::Error::InvalidScheme);
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapUri, _> = url.parse();

        assert_matches!(
            address.unwrap_err(),
            super::Error::UnexpectedUri(unknown) if &unknown == "redap/");
    }

    #[test]
    fn test_proxy_endpoint() {
        let url = "rerun://localhost:51234/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            origin: Origin {
                scheme: Scheme::RerunHttps,
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
    fn test_proxy_endpoint_with_space() {
        let url = "rerun http://127.0.0.1:9876/proxy";
        let address: Result<RedapUri, _> = url.parse();

        let expected = RedapUri::Proxy(ProxyUri {
            origin: Origin {
                scheme: Scheme::RerunHttp,
                host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                port: 9876,
            },
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
                        scheme: Scheme::RerunHttps,
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
                        scheme: Scheme::RerunHttps,
                        host: url::Host::Domain("example.com".to_owned()),
                        port: 443,
                    },
                }),
            ),
            (
                "rerun://example.com:420/catalog",
                RedapUri::Catalog(CatalogUri {
                    origin: Origin {
                        scheme: Scheme::RerunHttps,
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
                scheme: Scheme::RerunHttps,
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
                scheme: Scheme::RerunHttps,
                host: url::Host::Domain("localhost".to_owned()),
                port: 123,
            },
        });

        assert_eq!(url.parse::<RedapUri>().unwrap(), expected);
    }

    #[test]
    fn test_folder_endpoint_roundtrip() {
        let url = "rerun://localhost:51234/folder/perception.detection";
        let parsed: RedapUri = url.parse().unwrap();

        let RedapUri::Folder(folder_uri) = &parsed else {
            panic!("expected Folder variant, got {parsed:?}");
        };
        assert_eq!(folder_uri.path, "perception.detection");
        assert_eq!(folder_uri.origin.host.to_string(), "localhost");
        assert_eq!(folder_uri.origin.port, 51234);

        // Display → parse roundtrips back to the same URI.
        let displayed = parsed.to_string();
        let reparsed: RedapUri = displayed.parse().unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn test_folder_endpoint_percent_encoded() {
        // Path containing a `/` must be percent-encoded as `%2F` to survive a roundtrip.
        let url = "rerun://localhost:51234/folder/odd%2Fname";
        let parsed: RedapUri = url.parse().unwrap();

        let RedapUri::Folder(folder_uri) = &parsed else {
            panic!("expected Folder variant, got {parsed:?}");
        };
        assert_eq!(folder_uri.path, "odd/name");

        let reparsed: RedapUri = parsed.to_string().parse().unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn test_folder_endpoint_empty_path_rejected() {
        let url = "rerun://localhost:51234/folder/";
        let address: Result<RedapUri, _> = url.parse();
        assert!(address.is_err());
    }
}
