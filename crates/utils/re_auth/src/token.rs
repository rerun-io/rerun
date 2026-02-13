use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use jsonwebtoken::decode_header;

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("token does not seem to be a valid JWT token: {0}")]
    MalformedToken(#[source] jsonwebtoken::errors::Error),
}

/// Default `allowed_hosts` pattern for tokens that have no `allowed_hosts` claim.
pub const DEFAULT_ALLOWED_HOSTS: &str = ".cloud.rerun.io";

/// Environment variable to bypass the host check entirely.
pub const INSECURE_SKIP_HOST_CHECK_ENV: &str = "RERUN_INSECURE_SKIP_HOST_CHECK";

#[derive(Debug, thiserror::Error)]
#[error(
    "token is not allowed for host '{host}'; \
     set {INSECURE_SKIP_HOST_CHECK_ENV}=1 to override"
)]
pub struct HostMismatchError {
    pub host: String,
}

/// A JWT that is used to authenticate the client.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Jwt(pub(crate) String);

/// Error from decoding a JWT payload.
#[derive(Debug, thiserror::Error)]
pub enum JwtDecodeError {
    #[error("missing `.` separator between header and payload")]
    MissingHeaderPayloadSeparator,

    #[error("missing `.` separator between payload and signature")]
    MissingPayloadSignatureSeparator,

    #[error("failed to decode base64 payload: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("failed to deserialize payload: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Jwt {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode the JWT payload into any deserializable type without verifying
    /// the signature.
    pub fn decode_claims<T: serde::de::DeserializeOwned>(&self) -> Result<T, JwtDecodeError> {
        let (_header, rest) = self
            .0
            .split_once('.')
            .ok_or(JwtDecodeError::MissingHeaderPayloadSeparator)?;
        let (payload, _signature) = rest
            .split_once('.')
            .ok_or(JwtDecodeError::MissingPayloadSignatureSeparator)?;
        let payload = BASE64_URL_SAFE_NO_PAD.decode(payload)?;
        Ok(serde_json::from_slice(&payload)?)
    }

    /// Returns the token string only if its `allowed_hosts` claim permits `host`.
    ///
    /// Use this whenever sending a token to a remote server to prevent
    /// accidentally leaking tokens to unintended recipients.
    pub fn for_host(&self, host: &str) -> Result<&str, HostMismatchError> {
        if token_allowed_for_host(self, host) {
            Ok(&self.0)
        } else {
            Err(HostMismatchError {
                host: host.to_owned(),
            })
        }
    }
}

impl TryFrom<String> for Jwt {
    type Error = TokenError;

    fn try_from(token: String) -> Result<Self, Self::Error> {
        // We first check if the general structure of the token is correct.
        let token = token.trim();
        let _ = decode_header(token).map_err(Self::Error::MalformedToken)?;
        Ok(Self(token.to_owned()))
    }
}

impl std::fmt::Display for Jwt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- Host checking ---

/// Check if a hostname matches a host pattern.
///
/// Uses RFC 6265 (cookie) domain matching semantics:
/// - A leading `.` means "this domain and all subdomains":
///   `.cloud.rerun.io` matches `api.acme.cloud.rerun.io` and
///   `acme.cloud.rerun.io`, but not `cloud.rerun.io` itself
///   or `mycloud.rerun.io`.
/// - Without a leading `.`, an exact match is required.
///
/// Both sides are normalized via [`url::Host::parse`] (IDNA / lowercase / punycode)
/// before comparison, so `API.Acme.Cloud.Rerun.IO` matches `.cloud.rerun.io`.
pub fn host_matches_pattern(pattern: &str, host: &str) -> bool {
    // Normalize the host through url::Host::parse (IDNA lowercasing + punycode).
    // If parsing fails (e.g. empty string), fall back to the raw input.
    let host_normalized = url::Host::parse(host)
        .map(|h| h.to_string())
        .unwrap_or_else(|_| host.to_owned());

    if let Some(suffix) = pattern.strip_prefix('.') {
        let suffix_normalized = url::Host::parse(suffix)
            .map(|h| h.to_string())
            .unwrap_or_else(|_| suffix.to_owned());

        // Work at the DNS label level to avoid partial-label matches.
        let suffix_labels: Vec<&str> = suffix_normalized.split('.').collect();
        let host_labels: Vec<&str> = host_normalized.split('.').collect();
        // The host must have strictly more labels than the suffix
        // (i.e. at least one subdomain label).
        host_labels.len() > suffix_labels.len() && host_labels.ends_with(&suffix_labels)
    } else {
        let pattern_normalized = url::Host::parse(pattern)
            .map(|h| h.to_string())
            .unwrap_or_else(|_| pattern.to_owned());
        host_normalized == pattern_normalized
    }
}

/// Extract the `allowed_hosts` claim from a JWT without verifying its signature.
///
/// Handles both single-string and array-of-strings representations.
fn extract_allowed_hosts_from_jwt(jwt: &Jwt) -> Result<Vec<String>, JwtDecodeError> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        One(String),
        Many(Vec<String>),
    }

    #[derive(serde::Deserialize)]
    struct AllowedHostsOnly {
        #[serde(default)]
        allowed_hosts: Option<StringOrVec>,
    }

    let parsed: AllowedHostsOnly = jwt.decode_claims()?;

    Ok(match parsed.allowed_hosts {
        Some(StringOrVec::One(s)) => vec![s],
        Some(StringOrVec::Many(v)) => v,
        None => vec![],
    })
}

/// Check if a token's `allowed_hosts` claim permits the given host.
///
/// Works for both Rerun Cloud tokens (RS256, from `WorkOS`) and Redap
/// machine tokens (HS256, from `generate-token`).
///
/// Returns `true` if:
/// - The `RERUN_INSECURE_SKIP_HOST_CHECK` env var is set to `"1"`
/// - Any of the token's `allowed_hosts` values matches the host
///   (or [`DEFAULT_ALLOWED_HOSTS`] if no `allowed_hosts` claim is present)
///
/// Returns `false` if no pattern matches, meaning the token
/// should NOT be sent to this host.
pub fn token_allowed_for_host(jwt: &Jwt, host: &str) -> bool {
    if std::env::var(INSECURE_SKIP_HOST_CHECK_ENV).ok().as_deref() == Some("1") {
        re_log::debug!("{INSECURE_SKIP_HOST_CHECK_ENV} is set, skipping host check");
        return true;
    }

    let allowed_hosts = match extract_allowed_hosts_from_jwt(jwt) {
        Ok(hosts) => hosts,
        Err(err) => {
            re_log::debug!("failed to parse token for host check: {err}");
            return true;
        }
    };

    // If no allowed_hosts claim, fall back to the default.
    if allowed_hosts.is_empty() {
        return host_matches_pattern(DEFAULT_ALLOWED_HOSTS, host);
    }

    allowed_hosts
        .iter()
        .any(|pattern| host_matches_pattern(pattern, host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_pattern_domain_match() {
        // Typical customer deployments
        assert!(host_matches_pattern(
            ".cloud.rerun.io",
            "api.acme.cloud.rerun.io"
        ));
        assert!(host_matches_pattern(
            ".cloud.rerun.io",
            "api.bigcorp.cloud.rerun.io"
        ));
        // Dev environments
        assert!(host_matches_pattern(
            ".dev.rerun.io",
            "api.jleibs.dev.rerun.io"
        ));
    }

    #[test]
    fn test_host_pattern_domain_no_match() {
        // The domain itself should not match (need at least one subdomain)
        assert!(!host_matches_pattern(".cloud.rerun.io", "cloud.rerun.io"));
        // Partial label overlap must not match
        assert!(!host_matches_pattern(".cloud.rerun.io", "mycloud.rerun.io"));
        // Completely different host
        assert!(!host_matches_pattern(".cloud.rerun.io", "evil.com"));
        assert!(!host_matches_pattern(".cloud.rerun.io", "localhost"));
    }

    #[test]
    fn test_host_pattern_exact_match() {
        assert!(host_matches_pattern(
            "api.acme.cloud.rerun.io",
            "api.acme.cloud.rerun.io"
        ));
        assert!(!host_matches_pattern(
            "api.acme.cloud.rerun.io",
            "api.bigcorp.cloud.rerun.io"
        ));
    }

    #[test]
    fn test_host_pattern_case_insensitive() {
        assert!(host_matches_pattern(
            ".cloud.rerun.io",
            "API.Acme.Cloud.Rerun.IO"
        ));
        assert!(host_matches_pattern(
            ".CLOUD.RERUN.IO",
            "api.acme.cloud.rerun.io"
        ));
        assert!(host_matches_pattern(
            "API.ACME.CLOUD.RERUN.IO",
            "api.acme.cloud.rerun.io"
        ));
    }
}
