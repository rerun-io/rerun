use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::{Error, Jwt};

/// Identifies who should be the consumer of a token. In our case, this is the Rerun storage node.
const AUDIENCE: &str = "redap";

/// A secret key that is used to generate and verify tokens.
///
/// This represents a symmetric authentication scheme, which means that the
/// same key is used to both sign and verify the token.
/// In the future, we will need to support asymmetric schemes too.
///
/// The key is stored unencrypted in memory.
#[derive(Debug, Clone)]
pub struct RedapProvider {
    secret_key: SecretKey,

    #[cfg(feature = "oauth")]
    oauth: Option<RerunCloudProvider>,
}

#[cfg(feature = "oauth")]
#[derive(Debug, Clone)]
struct RerunCloudProvider {
    /// Public keys
    keys: jsonwebtoken::jwk::JwkSet,

    /// Expected organization ID
    org_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(Vec<u8>);

impl SecretKey {
    #[inline]
    pub fn reveal(&self) -> &[u8] {
        &self.0
    }

    /// Generates a new secret key.
    pub fn generate(rng: impl rand::Rng) -> Self {
        // 32 bytes or 256 bits
        let secret_key = generate_secret_key(rng, 32);

        debug_assert_eq!(
            secret_key.len() * size_of::<u8>() * 8,
            256,
            "The resulting secret should be 256 bits."
        );

        Self(secret_key)
    }

    /// Decodes a [`base64`] encoded secret key.
    pub fn from_base64(base64: impl AsRef<str>) -> Result<Self, Error> {
        Ok(Self(general_purpose::STANDARD.decode(base64.as_ref())?))
    }

    /// Encodes the secret key as a [`base64`] string.
    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(&self.0)
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("********")
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RedapClaims {
    /// The issuer of the token.
    ///
    /// Could be an identity provider or the storage node directly.
    pub iss: String,

    /// The subject (user) of the token.
    pub sub: String,

    /// The audience of the token, i.e. who should consume it.
    ///
    /// Most of the time this will be the storage node.
    /// Per RFC 7519, this can be either a single string or an array of strings.
    #[serde(
        deserialize_with = "deser_string_or_vec",
        serialize_with = "ser_string_or_vec"
    )]
    pub aud: Vec<String>,

    /// Expiry time of the token.
    pub exp: u64,

    /// Issued at time of the token.
    pub iat: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Claims {
    #[cfg(feature = "oauth")]
    RerunCloud(crate::oauth::RerunCloudClaims),

    Redap(RedapClaims),
}

impl Claims {
    /// Subject, usually the user ID.
    pub fn sub(&self) -> &str {
        match self {
            #[cfg(feature = "oauth")]
            Self::RerunCloud(claims) => claims.sub.as_str(),
            Self::Redap(claims) => claims.sub.as_str(),
        }
    }

    /// Issuer
    pub fn iss(&self) -> &str {
        match self {
            #[cfg(feature = "oauth")]
            Self::RerunCloud(claims) => claims.iss.as_str(),
            Self::Redap(claims) => claims.iss.as_str(),
        }
    }

    #[cfg(feature = "oauth")]
    pub fn permissions(&self) -> &[crate::oauth::Permission] {
        match self {
            Self::RerunCloud(claims) => &claims.permissions[..],
            Self::Redap(_) => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerificationOptions {
    leeway: Option<Duration>,
}

impl VerificationOptions {
    #[inline]
    pub fn with_leeway(mut self, leeway: Option<Duration>) -> Self {
        self.leeway = leeway;
        self
    }

    #[inline]
    pub fn without_leeway(mut self) -> Self {
        self.leeway = None;
        self
    }
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self {
            // 5 minutes to prevent clock skew
            leeway: Some(Duration::from_secs(5 * 60)),
        }
    }
}

#[derive(Clone, Copy)]
enum KeyProvider {
    #[cfg(feature = "oauth")]
    RerunCloud,
    Redap,
}

impl VerificationOptions {
    fn for_provider(self, provider: KeyProvider) -> Validation {
        match provider {
            #[cfg(feature = "oauth")]
            KeyProvider::RerunCloud => {
                let mut validation = Validation::new(Algorithm::RS256);
                validation.set_issuer(&[&*crate::oauth::OAUTH_ISSUER_URL]);
                validation.required_spec_claims.extend(
                    crate::oauth::RerunCloudClaims::REQUIRED
                        .iter()
                        .copied()
                        .map(String::from),
                );
                validation.validate_exp = true;
                validation.leeway = self.leeway.map_or(0, |leeway| leeway.as_secs());
                validation
            }
            KeyProvider::Redap => {
                let mut validation = Validation::new(Algorithm::HS256);
                validation.set_audience(&[AUDIENCE.to_owned()]);
                validation.set_required_spec_claims(&["exp", "sub", "aud", "iss"]);
                validation.leeway = self.leeway.map_or(0, |leeway| leeway.as_secs());
                validation
            }
        }
    }
}

// Generate a random secret key of specified length
fn generate_secret_key(mut rng: impl rand::Rng, length: usize) -> Vec<u8> {
    (0..length).map(|_| rng.random::<u8>()).collect()
}

impl RedapProvider {
    /// Create an authentication provider from a secret key.
    pub fn from_secret_key(secret_key: SecretKey) -> Self {
        Self {
            secret_key,
            #[cfg(feature = "oauth")]
            oauth: None,
        }
    }

    /// Create an authentication provider from a secret key encoded as base64.
    pub fn from_secret_key_base64(secret_key: &str) -> Result<Self, Error> {
        Ok(Self {
            secret_key: SecretKey::from_base64(secret_key)?,
            #[cfg(feature = "oauth")]
            oauth: None,
        })
    }

    /// Allow users from the given organization to authenticate via
    /// their Rerun Cloud credentials.
    #[cfg(feature = "oauth")]
    pub async fn with_rerun_cloud_provider(self, org_id: impl Into<String>) -> Result<Self, Error> {
        use crate::oauth::api;

        // TODO(jan): fetch these less often? cache somehow?
        let keys = api::jwks().await.map_err(|err| {
            re_log::debug!("failed to fetch external keys: {err}");
            Error::JwksFetch(err)
        })?;
        let org_id = org_id.into();

        let provider = RerunCloudProvider { keys, org_id };

        Ok(Self {
            secret_key: self.secret_key,
            oauth: Some(provider),
        })
    }

    /// Generates a new JWT token that is valid for the given duration.
    ///
    /// It is important to note that the token is not encrypted, but merely
    /// signed by the [`RedapProvider`]. This means that its contents are readable
    /// by everyone.
    ///
    /// If `duration` is `None`, the token will be valid forever. `scope` can be
    /// used to restrict the token to a specific context.
    pub fn token(
        &self,
        duration: Duration,
        issuer: impl Into<String>,
        subject: impl Into<String>,
    ) -> Result<Jwt, Error> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;

        let claims = Claims::Redap(RedapClaims {
            iss: issuer.into(),
            sub: subject.into(),
            aud: vec![AUDIENCE.to_owned()],
            exp: (now + duration).as_secs(),
            iat: now.as_secs(),
        });

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret_key.reveal()),
        )?;

        Ok(Jwt(token))
    }

    /// Checks if a provided `token` is valid for a given `scope`.
    pub fn verify(&self, token: &Jwt, options: VerificationOptions) -> Result<Claims, Error> {
        #[cfg(feature = "oauth")]
        let (key, validation) = if let Some(kid) = jsonwebtoken::decode_header(token.as_str())?.kid
        {
            // we don't supply key ID, so assume this comes from external provider
            let Some(provider) = &self.oauth else {
                return Err(Error::NoExternalProvider);
            };

            let Some(key) = provider.keys.find(&kid) else {
                re_log::debug!("no key with id {kid} found");
                return Err(Error::InvalidToken);
            };
            let key = DecodingKey::from_jwk(key)?;
            let validation = options.for_provider(KeyProvider::RerunCloud);
            (key, validation)
        } else {
            let key = DecodingKey::from_secret(self.secret_key.reveal());
            let validation = options.for_provider(KeyProvider::Redap);
            (key, validation)
        };

        #[cfg(not(feature = "oauth"))]
        let (key, validation) = {
            let key = DecodingKey::from_secret(self.secret_key.reveal());
            let validation = options.for_provider(KeyProvider::Redap);
            (key, validation)
        };

        let token_data = decode::<Claims>(&token.0, &key, &validation)?;

        match &token_data.claims {
            #[cfg(feature = "oauth")]
            Claims::RerunCloud(claims) => {
                let provider = self
                    .oauth
                    .as_ref()
                    .expect("bug: verified external key without external provider configured");
                if claims.org_id != provider.org_id {
                    re_log::debug!(
                        "verification failed: organization ID was not {}",
                        provider.org_id
                    );
                    return Err(Error::InvalidToken);
                }
            }
            Claims::Redap(_) => {
                // no additional verification
            }
        }

        Ok(token_data.claims)
    }
}

// ---

/// Deserializes either a string of an array of strings into an array of strings.
fn deser_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        One(String),
        Many(Vec<String>),
    }

    use serde::Deserialize as _;
    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::One(s) => Ok(vec![s]),
        StringOrVec::Many(v) => Ok(v),
    }
}

/// Serializes an array of strings into either a single string if unary, or into an array of strings otherwise.
fn ser_string_or_vec<S>(value: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize as _;
    if value.len() == 1 {
        serializer.serialize_str(&value[0])
    } else {
        value.serialize(serializer)
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audience_deserialize_single_string() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": "redap",
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, vec!["redap"]);
    }

    #[test]
    fn test_audience_deserialize_array() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": ["redap", "other-service"],
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, vec!["redap", "other-service"]);
    }

    #[test]
    fn test_audience_deserialize_empty_array() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": [],
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, Vec::<String>::new());
    }

    #[test]
    fn test_audience_serialize_single() {
        let claims = RedapClaims {
            iss: "test".to_owned(),
            sub: "user123".to_owned(),
            aud: vec!["redap".to_owned()],
            exp: 1234567890,
            iat: 1234567890,
        };

        let json = serde_json::to_value(&claims).unwrap();
        // When there's exactly one audience, it should serialize as a string
        assert_eq!(json["aud"], serde_json::json!("redap"));
    }

    #[test]
    fn test_audience_serialize_multiple() {
        let claims = RedapClaims {
            iss: "test".to_owned(),
            sub: "user123".to_owned(),
            aud: vec!["redap".to_owned(), "other".to_owned()],
            exp: 1234567890,
            iat: 1234567890,
        };

        let json = serde_json::to_value(&claims).unwrap();
        // When there are multiple audiences, it should serialize as an array
        assert_eq!(json["aud"], serde_json::json!(["redap", "other"]));
    }
}
