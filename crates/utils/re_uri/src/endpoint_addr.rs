use crate::Origin;

/// A Rerun endpoint address, consisting of an origin and an optional path prefix.
///
/// Example: `https://rerun.io:443/custom/prefix`
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EndpointAddr {
    pub origin: Origin,

    /// An optional path prefix, e.g. `/my/prefix`.
    ///
    /// The prefix is guaranteed to start with a slash if it is not empty,
    /// and guaranteed not to end with a slash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
}

impl EndpointAddr {
    /// Create a new [`EndpointAddr`] with the given origin and no path prefix.
    pub fn new(origin: Origin) -> Self {
        Self {
            origin,
            path_prefix: None,
        }
    }

    /// Add a path prefix to the endpoint address.
    ///
    /// The prefix is normalized:
    /// - It will be ensured to start with a `/` if not empty.
    /// - Trailing slashes will be removed.
    pub fn with_path_prefix(mut self, path_prefix: impl Into<String>) -> Self {
        let path_prefix = path_prefix.into();
        if path_prefix.is_empty() || path_prefix == "/" {
            self.path_prefix = None;
            return self;
        }

        let mut path_prefix = path_prefix.trim_end_matches('/').to_owned();
        if !path_prefix.starts_with('/') {
            path_prefix.insert(0, '/');
        }

        self.path_prefix = Some(path_prefix);
        self
    }
}

impl std::fmt::Display for EndpointAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            path_prefix,
        } = self;

        write!(f, "{origin}")?;
        if let Some(prefix) = path_prefix {
            write!(f, "{prefix}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scheme;

    fn test_origin() -> Origin {
        Origin::from_scheme_and_socket_addr(Scheme::Rerun, "127.0.0.1:1234".parse().unwrap())
    }

    #[test]
    fn test_serialization() {
        let origin = test_origin();
        let addr = EndpointAddr::new(origin.clone());
        assert_eq!(addr.path_prefix, None);
        assert_eq!(addr.to_string(), "rerun://127.0.0.1:1234");

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("");
        assert_eq!(addr.path_prefix, None);

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("/");
        assert_eq!(addr.path_prefix, None);

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("foo");
        assert_eq!(addr.path_prefix.as_deref(), Some("/foo"));
        assert_eq!(addr.to_string(), "rerun://127.0.0.1:1234/foo");

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("/foo");
        assert_eq!(addr.path_prefix.as_deref(), Some("/foo"));
        assert_eq!(addr.to_string(), "rerun://127.0.0.1:1234/foo");

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("foo/");
        assert_eq!(addr.path_prefix.as_deref(), Some("/foo"));
        assert_eq!(addr.to_string(), "rerun://127.0.0.1:1234/foo");

        let addr = EndpointAddr::new(origin.clone()).with_path_prefix("/foo/bar");
        assert_eq!(addr.path_prefix.as_deref(), Some("/foo/bar"));
        assert_eq!(addr.to_string(), "rerun://127.0.0.1:1234/foo/bar");
    }
}
