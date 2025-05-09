use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

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
#[derive(Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct RedapProvider {
    secret_key: Vec<u8>,
}

impl std::fmt::Debug for RedapProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedapProvider")
            .field("secret_key", &"********")
            .finish()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    /// The issuer of the token.
    ///
    /// Could be an identity provider or the storage node directly.
    pub iss: String,

    /// The subject (user) of the token.
    pub sub: String,

    /// The audience of the token, i.e. who should consume it.
    ///
    /// Most of the time this will be the storage node.
    pub aud: String,

    /// Expiry time of the token.
    pub exp: u64,

    /// Issued at time of the token.
    pub iat: u64,
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

impl From<VerificationOptions> for Validation {
    fn from(options: VerificationOptions) -> Self {
        let mut validation = Self::new(Algorithm::HS256);
        validation.set_audience(&[AUDIENCE.to_owned()]);
        validation.set_required_spec_claims(&["exp", "sub", "aud", "iss"]);
        validation.leeway = options.leeway.map_or(0, |leeway| leeway.as_secs());
        validation
    }
}

// Generate a random secret key of specified length
fn generate_secret_key(mut rng: impl rand::Rng, length: usize) -> Vec<u8> {
    (0..length).map(|_| rng.r#gen::<u8>()).collect()
}

impl RedapProvider {
    /// Generates a new secret key.
    pub fn generate(rng: impl rand::Rng) -> Self {
        // 32 bytes or 256 bits
        let secret_key = generate_secret_key(rng, 32);

        debug_assert_eq!(
            secret_key.len() * size_of::<u8>() * 8,
            256,
            "The resulting secret should be 256 bits."
        );

        Self { secret_key }
    }

    /// Decodes a [`base64`] encoded secret key.
    pub fn from_base64(base64: impl AsRef<str>) -> Result<Self, Error> {
        let secret_key = general_purpose::STANDARD.decode(base64.as_ref())?;
        Ok(Self { secret_key })
    }

    /// Encodes the secret key as a [`base64`] string.
    pub fn to_base64(&self) -> String {
        general_purpose::STANDARD.encode(&self.secret_key)
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

        let claims = Claims {
            iss: issuer.into(),
            sub: subject.into(),
            aud: AUDIENCE.to_owned(),
            exp: (now + duration).as_secs(),
            iat: now.as_secs(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.secret_key),
        )?;

        Ok(Jwt(token))
    }

    /// Checks if a provided `token` is valid for a given `scope`.
    pub fn verify(&self, token: &Jwt, options: VerificationOptions) -> Result<Claims, Error> {
        let validation = options.into();

        let token_data = decode::<Claims>(
            &token.0,
            &DecodingKey::from_secret(&self.secret_key),
            &validation,
        )?;

        Ok(token_data.claims)
    }
}
