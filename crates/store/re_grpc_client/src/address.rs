use url::Url;

/// The given url is not a valid Rerun storage node URL.
#[derive(thiserror::Error, Debug)]
#[error("URL {url:?} should follow rerun://addr:port/recording/12345 for recording or rerun://addr:port/catalog for catalog")]
pub struct InvalidRedapAddress {
    url: Url,
    msg: String,
}

/// Parsed `rerun://addr:port/recording/12345`
pub struct RecordingAddress {
    pub redap_endpoint: Url,
    pub recording_id: String,
}

impl std::fmt::Display for RecordingAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingAddress {{ redap_endpoint: {}, recording_id: {} }}",
            self.redap_endpoint, self.recording_id
        )
    }
}

impl TryFrom<Url> for RecordingAddress {
    type Error = InvalidRedapAddress;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if url.scheme() != "rerun" {
            return Err(InvalidRedapAddress {
                url: url.clone(),
                msg: "Invalid scheme, expected 'rerun'".to_owned(),
            });
        }

        let redap_endpoint = get_redap_endpoint(&url)?;

        let path_segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
        if path_segments.len() != 2 || path_segments[0] != "recording" {
            return Err(InvalidRedapAddress {
                url: url.clone(),
                msg: "Invalid path, expected '/recording/{id}'".to_owned(),
            });
        }

        Ok(Self {
            redap_endpoint,
            recording_id: path_segments[1].to_owned(),
        })
    }
}

/// Parsed `rerun://addr:port/catalog`
pub struct CatalogAddress {
    pub redap_endpoint: Url,
}

impl std::fmt::Display for CatalogAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReDap endpoint {}", self.redap_endpoint)
    }
}

impl TryFrom<Url> for CatalogAddress {
    type Error = InvalidRedapAddress;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if url.scheme() != "rerun" {
            return Err(InvalidRedapAddress {
                url: url.clone(),
                msg: "Invalid scheme, expected 'rerun'".to_owned(),
            });
        }

        let redap_endpoint = get_redap_endpoint(&url)?;

        let path_segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
        if path_segments.len() != 1 || path_segments[0] != "catalog" {
            return Err(InvalidRedapAddress {
                url: url.clone(),
                msg: "Invalid path, expected '/catalog'".to_owned(),
            });
        }

        Ok(Self { redap_endpoint })
    }
}

/// Small helper to extract host and port from the Rerun Data Platform URL.
fn get_redap_endpoint(url: &Url) -> Result<Url, InvalidRedapAddress> {
    let host = url.host_str().ok_or_else(|| InvalidRedapAddress {
        url: url.clone(),
        msg: "Missing host".to_owned(),
    })?;

    if host == "0.0.0.0" {
        re_log::warn!("Using 0.0.0.0 as Rerun Data Platform host will often fail. You might want to try using 127.0.0.0.");
    }

    let port = url.port().ok_or_else(|| InvalidRedapAddress {
        url: url.clone(),
        msg: "Missing port".to_owned(),
    })?;

    #[allow(clippy::unwrap_used)]
    let redap_endpoint = Url::parse(&format!("http://{host}:{port}")).unwrap();

    Ok(redap_endpoint)
}
