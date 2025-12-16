use crate::Error;

/// The different schemes supported by Rerun.
///
/// We support `rerun`, `rerun+http`, and `rerun+https`.
#[derive(
    Debug, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    pub(crate) fn as_http_scheme(&self) -> &str {
        match self {
            Self::Rerun | Self::RerunHttps => "https",
            Self::RerunHttp => "http",
        }
    }

    /// Converts a rerun url into a canonical http or https url.
    pub(crate) fn canonical_url(&self, url: &str) -> String {
        match self {
            Self::Rerun => {
                debug_assert!(url.starts_with("rerun://"));
                url.replace("rerun://", "https://")
            }
            Self::RerunHttp => {
                debug_assert!(url.starts_with("rerun+http://"));
                url.replace("rerun+http://", "http://")
            }
            Self::RerunHttps => {
                debug_assert!(url.starts_with("rerun+https://"));
                url.replace("rerun+https://", "https://")
            }
        }
    }
}

impl std::str::FromStr for Scheme {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        if url.starts_with("rerun://") {
            Ok(Self::Rerun)
        } else if url.starts_with("rerun+http://") {
            Ok(Self::RerunHttp)
        } else if url.starts_with("rerun+https://") {
            Ok(Self::RerunHttps)
        } else {
            Err(crate::Error::InvalidScheme)
        }
    }
}
