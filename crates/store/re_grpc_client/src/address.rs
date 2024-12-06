use url::Url;

/// The given url is not a valid Rerun storage node URL.
#[derive(thiserror::Error, Debug)]
#[error("URL {url:?} should follow rerun://addr:port/recording/12345 for recording or rerun://addr:port/catalog for catalog")]
pub struct InvalidRedapAddress {
    url: String,
    msg: String,
}

/// Parsed from `rerun://addr:port/recording/12345` or `rerun://addr:port/catalog`
#[derive(PartialEq, Eq, Debug)]
pub enum RedapAddress {
    Recording {
        redap_endpoint: Url,
        recording_id: String,
    },
    Catalog {
        redap_endpoint: Url,
    },
}

impl std::fmt::Display for RedapAddress {
    #[allow(clippy::unwrap_used)] // host and port have already been verified during conversion
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording {
                redap_endpoint,
                recording_id,
            } => write!(
                f,
                "rerun://{}:{}/recording/{}",
                redap_endpoint.host().unwrap(),
                redap_endpoint.port().unwrap(),
                recording_id
            ),
            Self::Catalog { redap_endpoint } => write!(
                f,
                "rerun://{}:{}/catalog",
                redap_endpoint.host().unwrap(),
                redap_endpoint.port().unwrap(),
            ),
        }
    }
}

impl TryFrom<&str> for RedapAddress {
    type Error = InvalidRedapAddress;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let url = Url::parse(value).map_err(|err| InvalidRedapAddress {
            url: value.to_owned(),
            msg: err.to_string(),
        })?;

        if url.scheme() != "rerun" {
            return Err(InvalidRedapAddress {
                url: url.to_string(),
                msg: "Invalid scheme, expected 'rerun'".to_owned(),
            });
        }

        let host = url.host_str().ok_or_else(|| InvalidRedapAddress {
            url: url.to_string(),
            msg: "Missing host".to_owned(),
        })?;

        if host == "0.0.0.0" {
            re_log::warn!("Using 0.0.0.0 as Rerun Data Platform host will often fail. You might want to try using 127.0.0.0.");
        }

        let port = url.port().ok_or_else(|| InvalidRedapAddress {
            url: url.to_string(),
            msg: "Missing port".to_owned(),
        })?;

        #[allow(clippy::unwrap_used)]
        let redap_endpoint = Url::parse(&format!("http://{host}:{port}")).unwrap();

        // we got the ReDap endpoint, now figur out from the URL path if it's a recording or catalog
        if url.path().ends_with("/catalog") {
            let path_segments: Vec<&str> =
                url.path_segments().map(|s| s.collect()).unwrap_or_default();
            if path_segments.len() != 1 || path_segments[0] != "catalog" {
                return Err(InvalidRedapAddress {
                    url: url.to_string(),
                    msg: "Invalid path, expected '/catalog'".to_owned(),
                });
            }

            Ok(Self::Catalog { redap_endpoint })
        } else {
            let path_segments: Vec<&str> =
                url.path_segments().map(|s| s.collect()).unwrap_or_default();
            if path_segments.len() != 2 || path_segments[0] != "recording" {
                return Err(InvalidRedapAddress {
                    url: url.to_string(),
                    msg: "Invalid path, expected '/recording/{id}'".to_owned(),
                });
            }

            Ok(Self::Recording {
                redap_endpoint,
                recording_id: path_segments[1].to_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {

    use super::RedapAddress;
    use url::Url;

    #[test]
    fn test_recording_url_to_address() {
        let url = "rerun://0.0.0.0:1234/recording/12345";
        let address: RedapAddress = url.try_into().unwrap();
        assert_eq!(
            address,
            RedapAddress::Recording {
                redap_endpoint: Url::parse("http://0.0.0.0:1234").unwrap(),
                recording_id: "12345".to_owned()
            }
        );
    }

    #[test]
    fn test_catalog_url_to_address() {
        let url = "rerun://127.0.0.1:50051/catalog";
        let address: RedapAddress = url.try_into().unwrap();
        assert_eq!(
            address,
            RedapAddress::Catalog {
                redap_endpoint: Url::parse("http://127.0.0.1:50051").unwrap(),
            }
        );
    }

    #[test]
    fn test_invalid_url() {
        let url = "http://wrong-scheme:1234/recording/12345";
        let address: Result<RedapAddress, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::InvalidRedapAddress { .. }
        ));
    }

    #[test]
    fn test_invalid_path() {
        let url = "rerun://0.0.0.0:51234/redap/recordings/12345";
        let address: Result<RedapAddress, _> = url.try_into();

        assert!(matches!(
            address.unwrap_err(),
            super::InvalidRedapAddress { .. }
        ));
    }
}
