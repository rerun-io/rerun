/// The given url is not a valid Rerun storage node URL.
#[derive(thiserror::Error, Debug)]
#[error("URL {url:?} should follow rerun://addr:port/recording/12345")]
pub struct InvalidAddressError {
    url: String,
    msg: String,
}

/// Parsed `rerun://addr:port/recording/12345`
pub struct Address {
    pub addr_port: String,
    pub recording_id: String,
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rerun://{}/recording/{}",
            self.addr_port, self.recording_id
        )
    }
}

impl std::str::FromStr for Address {
    type Err = InvalidAddressError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let Some(stripped_url) = url.strip_prefix("rerun://") else {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Missing rerun://".to_owned(),
            });
        };

        let parts = stripped_url.split('/').collect::<Vec<_>>();
        if parts.len() < 3 {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Too few slashes".to_owned(),
            });
        }
        if parts.len() > 3 {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Too many slashes".to_owned(),
            });
        }

        if parts[1] != "recording" {
            return Err(InvalidAddressError {
                url: url.to_owned(),
                msg: "Not a recording".to_owned(),
            });
        }

        let addr_port = parts[0].to_owned();
        let recording_id = parts[2].to_owned();

        Ok(Self {
            addr_port,
            recording_id,
        })
    }
}
