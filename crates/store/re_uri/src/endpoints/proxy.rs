use crate::{Origin, RedapUri};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProxyUri {
    pub origin: Origin,

    /// Optional path prefix the proxy is mounted behind, e.g. `Some("foo/bar")`
    /// for `rerun+http://host/foo/bar/proxy`. `None` for the common
    /// `rerun+http://host/proxy`. Lets the message proxy be hosted behind a
    /// reverse proxy at a sub-path; the prefix is preserved when building the
    /// gRPC client base URL so requests go to `…/foo/bar/<service>/<method>`.
    pub path: Option<String>,
}

impl std::fmt::Display for ProxyUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}/{path}/proxy", self.origin),
            None => write!(f, "{}/proxy", self.origin),
        }
    }
}

impl ProxyUri {
    pub fn new(origin: Origin) -> Self {
        Self { origin, path: None }
    }

    /// Set the path prefix the proxy is mounted behind (see [`Self::path`]).
    #[inline]
    pub fn with_path(mut self, path: Option<String>) -> Self {
        self.path = path;
        self
    }

    /// The `http(s)://host:port[/prefix]` base URL the gRPC client should target.
    ///
    /// Unlike [`Origin::as_url`] this includes the optional mount [`Self::path`],
    /// so gRPC-web requests are issued to `…/<prefix>/<service>/<method>`.
    pub fn base_url(&self) -> String {
        match &self.path {
            Some(path) => format!("{}/{path}", self.origin.as_url()),
            None => self.origin.as_url(),
        }
    }
}

impl std::str::FromStr for ProxyUri {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Proxy(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(crate::Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// Serialize as string:
impl serde::Serialize for ProxyUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ProxyUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}
