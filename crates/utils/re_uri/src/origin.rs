use crate::{Error, Scheme};

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

    /// Parses a URL and returns the [`crate::Origin`] and the canonical URL (i.e. one that
    ///  starts with `http://` or `https://`).
    pub(crate) fn replace_and_parse(value: &str) -> Result<(Self, url::Url), Error> {
        let scheme: Scheme = value.parse()?;
        let rewritten = scheme.canonical_url(value);

        // We have to first rewrite the endpoint, because `Url` does not allow
        // `.set_scheme()` for non-opaque origins, nor does it return a proper
        // `Origin` in that case.
        let mut http_url = url::Url::parse(&rewritten)?;

        // If we parse a Url from e.g. `https://redap.rerun.io:443`, `port` in the Url struct will
        // be `None`. So we need to use `port_or_known_default` to get the port back.
        // See also: https://github.com/servo/rust-url/issues/957
        if http_url.port_or_known_default().is_none() {
            // If no port is specified, we assume the default redap port:
            http_url.set_port(Some(51234)).ok();
        }

        let url::Origin::Tuple(_, host, port) = http_url.origin() else {
            return Err(Error::UnexpectedOpaqueOrigin(value.to_owned()));
        };

        let origin = Self { scheme, host, port };

        Ok((origin, http_url))
    }
}

impl std::str::FromStr for Origin {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::replace_and_parse(value).map(|(origin, _)| origin)
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}
