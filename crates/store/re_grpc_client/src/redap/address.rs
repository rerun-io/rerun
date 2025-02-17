//! Rerun uses its own URL scheme to access information across the network.
//!
//! The following schemes are supported: `rerun+http://`, `rerun+https://` and
//! `rerun://`, which is an alias for `rerun+https://`. These schemes are then
//! converted on the fly to either `http://` or `https://`.

use re_protos::remote_store::v0::storage_node_client::StorageNodeClient;
use std::net::Ipv4Addr;

#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Connection error: {0}")]
    Tonic(#[from] tonic::transport::Error),

    #[error(transparent)]
    ParseError(#[from] url::ParseError),

    #[error("server is expecting an unencrypted connection (try `rerun+http://` if you are sure)")]
    UnencryptedServer,

    #[error("invalid or missing scheme (expected `rerun(+http|+https)://`)")]
    InvalidScheme,

    #[error("unexpected endpoint: {0}")]
    UnexpectedEndpoint(String),

    #[error("unexpected opaque origin: {0}")]
    UnexpectedOpaqueOrigin(String),

    #[error("unexpected base URL: {0}")]
    UnexpectedBaseUrl(String),

    /// The given url is not a valid Rerun storage node URL.
    #[error("URL {url:?} should follow rerun://host:port/recording/12345 for recording or rerun://host:port/catalog for catalog")]
    InvalidAddress { url: String, msg: String },
}

/// The different schemes supported by Rerun.
///
/// We support `rerun`, `rerun+http`, and `rerun+https`.
#[derive(Debug, PartialEq, Eq)]
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
    fn to_http_scheme(&self) -> &str {
        match self {
            Self::Rerun | Self::RerunHttps => "https",
            Self::RerunHttp => "http",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Origin {
    scheme: Scheme,
    host: url::Host<String>,
    port: u16,
}

impl Origin {
    // TODO(#8411): figure out the right size for this
    const MAX_DECODING_MESSAGE_SIZE: usize = usize::MAX;

    // Converts an entire [`Origin`] to a `http` or `https` URL.
    fn to_http_scheme(&self) -> String {
        format!(
            "{}://{}:{}",
            self.scheme.to_http_scheme(),
            self.host,
            self.port
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn coerce_http_scheme(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn client(
        &self,
    ) -> Result<StorageNodeClient<tonic_web_wasm_client::Client>, ConnectionError> {
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            self.to_http_scheme(),
            tonic_web_wasm_client::options::FetchOptions::new(),
        );

        Ok(StorageNodeClient::new(tonic_client)
            .max_decoding_message_size(Self::MAX_DECODING_MESSAGE_SIZE))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn client(
        &self,
    ) -> Result<StorageNodeClient<tonic::transport::Channel>, ConnectionError> {
        use tonic::transport::Endpoint;

        match Endpoint::new(self.to_http_scheme())?
            .tls_config(tonic::transport::ClientTlsConfig::new().with_enabled_roots())?
            .connect()
            .await
        {
            Ok(client) => Ok(StorageNodeClient::new(client)
                .max_decoding_message_size(Self::MAX_DECODING_MESSAGE_SIZE)),
            Err(original_error) => {
                // If we can't establish a connection, we probe if the server is
                // expecting unencrypted traffic. If that is the case, we return
                // a more meaningful error message.
                let Ok(endpoint) = Endpoint::new(self.coerce_http_scheme()) else {
                    return Err(ConnectionError::Tonic(original_error));
                };

                if endpoint.connect().await.is_ok() {
                    Err(ConnectionError::UnencryptedServer)
                } else {
                    Err(ConnectionError::Tonic(original_error))
                }
            }
        }
    }
}

/// Parses a URL and returns the [`Origin`] and the canonical URL (i.e. one that
///  starts with `http://` or `https://`).
fn replace_and_parse(value: &str) -> Result<(Origin, url::Url), ConnectionError> {
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
        Err(ConnectionError::InvalidScheme)
    }?;

    // We have to first rewrite the endpoint, because `Url` does not allow
    // `.set_scheme()` for non-opaque origins, nor does it return a proper
    // `Origin` in that case.
    let canonic_url = url::Url::parse(&rewritten)?;

    let url::Origin::Tuple(_, host, port) = canonic_url.origin() else {
        return Err(ConnectionError::UnexpectedOpaqueOrigin(value.to_owned()));
    };

    if host == url::Host::<String>::Ipv4(Ipv4Addr::UNSPECIFIED) {
        re_log::warn!("Using 0.0.0.0 as Rerun Data Platform host will often fail. You might want to try using 127.0.0.0.");
    }

    let origin = Origin { scheme, host, port };

    Ok((origin, canonic_url))
}

impl TryFrom<&str> for Origin {
    type Error = ConnectionError;

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
    type Error = ConnectionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (origin, canonical_url) = replace_and_parse(value)?;

        // :warning: We limit the amount of segments, which might need to be
        // adjusted when adding additional resources.
        let segments = canonical_url
            .path_segments()
            .ok_or_else(|| ConnectionError::UnexpectedBaseUrl(value.to_owned()))?
            .take(2)
            .collect::<Vec<_>>();

        match segments.as_slice() {
            ["recording", recording_id] => Ok(Self::Recording {
                origin,
                recording_id: (*recording_id).to_owned(),
            }),
            ["catalog"] | [] => Ok(Self::Catalog { origin }),
            [unknown, ..] => Err(ConnectionError::UnexpectedEndpoint(format!("{unknown}/"))),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use core::net::Ipv4Addr;

    #[test]
    fn scheme_conversion() {
        assert_eq!(Scheme::Rerun.to_http_scheme(), "https");
        assert_eq!(Scheme::RerunHttp.to_http_scheme(), "http");
        assert_eq!(Scheme::RerunHttps.to_http_scheme(), "https");
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
            super::ConnectionError::InvalidScheme { .. }
        ));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapAddress, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::ConnectionError::UnexpectedEndpoint { .. }
        ));
    }
}
