//! Rerun uses its own URL scheme to access information across the network.
//!
//! The following schemes are supported: `rerun+http://`, `rerun+https://` and
//! `rerun://`, which is an alias for `rerun+https://`. These schemes are then
//! converted on the fly to either `http://` or `https://`.

use std::net::Ipv4Addr;

/// The given url is not a valid Rerun storage node URL.
#[derive(thiserror::Error, Debug)]
pub enum AddressError {
    #[error("URL {url:?} should follow rerun://host:port/recording/12345 for recording or rerun://host:port/catalog for catalog")]
    InvalidRedapAddress { url: String, msg: String },

    #[error("Catalog URL {origin:?} cannot be loaded as a recording")]
    CannotLoadCatalogAsRecording { origin: Origin },
}

/// The different schemes supported by Rerun.
///
/// We support `rerun`, `rerun+http`, and `rerun+https`.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
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
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Origin {
    scheme: Scheme,
    host: url::Host<String>,
    port: u16,
}

impl Origin {
    // Converts an entire [`Origin`] to a `http` or `https` URL.
    pub fn to_http_scheme(&self) -> String {
        format!(
            "{}://{}:{}",
            self.scheme.as_http_scheme(),
            self.host,
            self.port
        )
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(Debug, PartialEq, Eq)]
pub enum RedapAddress {
    Recording {
        origin: Origin,
        recording_id: String,
    },
    Catalog {
        origin: Origin,
    },
}

impl std::fmt::Display for RedapAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording {
                origin,
                recording_id,
            } => write!(f, "{origin}/recording/{recording_id}",),
            Self::Catalog { origin } => write!(f, "{origin}/catalog",),
        }
    }
}

impl TryFrom<&str> for RedapAddress {
    type Error = AddressError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (scheme, rewritten) = if value.starts_with("rerun://") {
            Ok((Scheme::Rerun, value.replace("rerun://", "https://")))
        } else if value.starts_with("rerun+http://") {
            Ok((Scheme::RerunHttp, value.replace("rerun+http://", "http://")))
        } else if value.starts_with("rerun+https://") {
            Ok((
                Scheme::RerunHttps,
                value.replace("rerun+https://", "https://"),
            ))
        } else {
            Err(AddressError::InvalidRedapAddress {
                url: value.to_owned(),
                msg: "Invalid scheme, expected `rerun://`,`rerun+http://`, or `rerun+https://`"
                    .to_owned(),
            })
        }?;

        // We have to first rewrite the endpoint, because `Url` does not allow
        // `.set_scheme()` for non-opaque origins, nor does it return a proper
        // `Origin` in that case.
        let redap_endpoint =
            url::Url::parse(&rewritten).map_err(|err| AddressError::InvalidRedapAddress {
                url: value.to_owned(),
                msg: err.to_string(),
            })?;

        let url::Origin::Tuple(_, host, port) = redap_endpoint.origin() else {
            return Err(AddressError::InvalidRedapAddress {
                url: value.to_owned(),
                msg: "Opaque origin".to_owned(),
            });
        };

        if host == url::Host::<String>::Ipv4(Ipv4Addr::UNSPECIFIED) {
            re_log::warn!("Using 0.0.0.0 as Rerun Data Platform host will often fail. You might want to try using 127.0.0.0.");
        }

        let origin = Origin { scheme, host, port };

        // :warning: We limit the amount of segments, which might need to be
        // adjusted when adding additional resources.
        let segments = redap_endpoint
            .path_segments()
            .ok_or_else(|| AddressError::InvalidRedapAddress {
                url: value.to_owned(),
                msg: "Cannot be a base URL".to_owned(),
            })?
            .take(2)
            .collect::<Vec<_>>();

        match segments.as_slice() {
            ["recording", recording_id] => Ok(Self::Recording {
                origin,
                recording_id: (*recording_id).to_owned(),
            }),
            ["catalog"] => Ok(Self::Catalog { origin }),

            _ => Err(AddressError::InvalidRedapAddress {
                url: value.to_owned(),
                msg: "Missing path'".to_owned(),
            }),
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
        assert_eq!(origin.to_http_scheme(), "https://127.0.0.1:1234");

        let origin = Origin {
            scheme: Scheme::RerunHttp,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.to_http_scheme(), "http://127.0.0.1:1234");

        let origin = Origin {
            scheme: Scheme::RerunHttps,
            host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
            port: 1234,
        };
        assert_eq!(origin.to_http_scheme(), "https://127.0.0.1:1234");
    }

    #[test]
    fn test_recording_url_to_address() {
        let url = "rerun://127.0.0.1:1234/recording/12345";
        let address: RedapAddress = url.try_into().unwrap();

        let RedapAddress::Recording {
            origin,
            recording_id,
        } = address
        else {
            panic!("Expected recording");
        };

        assert_eq!(origin.scheme, Scheme::Rerun);
        assert_eq!(origin.host, url::Host::<String>::Ipv4(Ipv4Addr::LOCALHOST));
        assert_eq!(origin.port, 1234);
        assert_eq!(recording_id, "12345");
    }

    #[test]
    fn test_http_catalog_url_to_address() {
        let url = "rerun+http://127.0.0.1:50051/catalog";
        let address: RedapAddress = url.try_into().unwrap();
        assert!(matches!(
            address,
            RedapAddress::Catalog {
                origin: Origin {
                    scheme: Scheme::RerunHttp,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
                },
            }
        ));
    }

    #[test]
    fn test_https_catalog_url_to_address() {
        let url = "rerun+https://127.0.0.1:50051/catalog";
        let address: RedapAddress = url.try_into().unwrap();

        assert!(matches!(
            address,
            RedapAddress::Catalog {
                origin: Origin {
                    scheme: Scheme::RerunHttps,
                    host: url::Host::Ipv4(Ipv4Addr::LOCALHOST),
                    port: 50051
                },
            }
        ));
    }

    #[test]
    fn test_invalid_url() {
        let url = "http://wrong-scheme:1234/recording/12345";
        let address: Result<RedapAddress, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::AddressError::InvalidRedapAddress { .. }
        ));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapAddress, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::AddressError::InvalidRedapAddress { .. }
        ));
    }
}
