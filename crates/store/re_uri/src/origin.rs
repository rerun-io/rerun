use std::net::SocketAddr;

use crate::{Error, Scheme};

/// `scheme://hostname:port`
#[derive(
    Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Origin {
    pub scheme: Scheme,
    pub host: url::Host<String>,
    pub port: u16,
}

impl Origin {
    pub fn from_scheme_and_socket_addr(scheme: Scheme, socket_addr: SocketAddr) -> Self {
        Self {
            scheme,
            host: match socket_addr.ip() {
                std::net::IpAddr::V4(ipv4_addr) => url::Host::Ipv4(ipv4_addr),
                std::net::IpAddr::V6(ipv6_addr) => url::Host::Ipv6(ipv6_addr),
            },
            port: socket_addr.port(),
        }
    }

    pub fn is_localhost(&self) -> bool {
        is_host_localhost(&self.host)
    }

    /// Converts the [`Origin`] to a URL that starts with either `http` or `https`.
    ///
    /// This is the URL to connect to. An ip of "0.0.0.0" will be shown as "127.0.0.1".
    pub fn as_url(&self) -> String {
        let Self { scheme, host, port } = self;
        let host = format_host(host);
        format!("{}://{host}:{port}", scheme.as_http_scheme())
    }

    pub fn format_host(&self) -> String {
        format_host(&self.host)
    }

    /// Converts the [`Origin`] to a `http` URL.
    ///
    /// In most cases you want to use [`Origin::as_url()`] instead.
    pub fn coerce_http_url(&self) -> String {
        let Self {
            scheme: _,
            host,
            port,
        } = self;
        let host = format_host(host);
        format!("http://{host}:{port}")
    }

    /// Parses a URL and returns the [`crate::Origin`] and the canonical URL (i.e. one that
    ///  starts with `http://` or `https://`).
    pub(crate) fn replace_and_parse(
        input: &str,
        default_localhost_port: Option<u16>,
    ) -> Result<(Self, url::Url), Error> {
        let (scheme, rewritten) = if !input.contains("://")
            && (input.contains("localhost") || input.contains("127.0.0.1"))
        {
            // Assume `rerun+http://`, because that is the default for localhost
            (Scheme::RerunHttp, format!("http://{input}"))
        } else {
            let scheme: Scheme = input.parse()?;
            (scheme, scheme.canonical_url(input))
        };

        // We have to first rewrite the endpoint, because `Url` does not allow
        // `.set_scheme()` for non-opaque origins, nor does it return a proper
        // `Origin` in that case.
        let mut http_url = url::Url::parse(&rewritten)?;

        let default_port = if is_origin_localhost(&http_url.origin()) {
            default_localhost_port
        } else if rewritten.starts_with("https://") {
            Some(443)
        } else if rewritten.starts_with("http://") {
            Some(80)
        } else {
            None
        };

        if let Some(default_port) = default_port {
            // Parsing with a non-standard scheme is a hack to work around the `url` crate bug.
            // TODO(servo/rust-url#706): stop doing this when the bug is fixed.
            let has_port = if let Some(rest) = http_url.to_string().strip_prefix("http://") {
                url::Url::parse(&format!("foobarbaz://{rest}"))?
                    .port()
                    .is_some()
            } else if let Some(rest) = http_url.to_string().strip_prefix("https://") {
                url::Url::parse(&format!("foobarbaz://{rest}"))?
                    .port()
                    .is_some()
            } else {
                true // Should not happen.
            };

            if !has_port {
                http_url.set_port(Some(default_port)).ok();
            }
        }

        let url::Origin::Tuple(_, host, port) = http_url.origin() else {
            return Err(Error::UnexpectedOpaqueOrigin(input.to_owned()));
        };

        let origin = Self { scheme, host, port };

        Ok((origin, http_url))
    }
}

impl std::str::FromStr for Origin {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::replace_and_parse(value, None).map(|(origin, _)| origin)
    }
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { scheme, host, port } = self;
        let host = format_host(host);
        write!(f, "{scheme}://{host}:{port}")
    }
}

fn format_host(host: &url::Host<String>) -> String {
    if is_host_unspecified(host) {
        // We usually cannot connect to "0.0.0.0" so we swap it for:
        "127.0.0.1".to_owned()
    } else {
        host.to_string()
    }
}

fn is_host_unspecified(host: &url::Host) -> bool {
    match host {
        url::Host::Domain(_domain) => false,
        url::Host::Ipv4(ip) => ip.is_unspecified(),
        url::Host::Ipv6(ip) => ip.is_unspecified(),
    }
}

fn is_origin_localhost(origin: &url::Origin) -> bool {
    match origin {
        url::Origin::Opaque(_) => false,
        url::Origin::Tuple(_, host, _) => is_host_localhost(host),
    }
}

fn is_host_localhost(host: &url::Host) -> bool {
    match host {
        url::Host::Domain(domain) => domain == "localhost",
        url::Host::Ipv4(ip) => ip.is_loopback() || ip.is_unspecified(),
        url::Host::Ipv6(ip) => ip.is_loopback() || ip.is_unspecified(),
    }
}

#[test]
fn test_origin_format() {
    assert_eq!(
        Origin::from_scheme_and_socket_addr(Scheme::Rerun, "192.168.0.2:1234".parse().unwrap())
            .to_string(),
        "rerun://192.168.0.2:1234"
    );
    assert_eq!(
        Origin::from_scheme_and_socket_addr(Scheme::Rerun, "0.0.0.0:1234".parse().unwrap())
            .to_string(),
        "rerun://127.0.0.1:1234"
    );
}
