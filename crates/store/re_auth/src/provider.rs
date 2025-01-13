use std::{collections::HashSet, time::Duration};

use base64::{engine::general_purpose, Engine as _};
use jwt_simple::{
    claims::{Claims, JWTClaims, NoCustomClaims},
    common::VerificationOptions,
    prelude::{HS256Key, MACLike as _},
};

pub type JwtClaims = JWTClaims<NoCustomClaims>;

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
#[derive(Clone)]
#[repr(transparent)]
pub struct RedapProvider {
    secret_key: HS256Key,
}

impl std::fmt::Debug for RedapProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedapProvider")
            .field("secret_key", &"********")
            .finish()
    }
}

impl RedapProvider {
    /// Generates a new secret key.
    pub fn generate() -> Self {
        let secret_key = HS256Key::generate();
        Self { secret_key }
    }

    /// Restores a secret key from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let secret_key = HS256Key::from_bytes(bytes);
        Self { secret_key }
    }

    /// Encodes the secret key as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.secret_key.to_bytes()
    }

    /// Decodes a [`base64`] encoded secret key.
    pub fn from_base64(base64: impl AsRef<str>) -> Result<Self, Error> {
        let bytes = general_purpose::STANDARD
            .decode(base64.as_ref())
            .map_err(Error::InvalidBase64)?;

        let secret_key = HS256Key::from_bytes(&bytes);
        Ok(Self { secret_key })
    }

    /// Encodes the secret key as a [`base64`] string.
    pub fn to_base64(&self) -> String {
        let bytes = self.secret_key.to_bytes();
        general_purpose::STANDARD.encode(&bytes)
    }

    /// Generates a new JWT token that is valid for the given duration.
    ///
    /// It is important to note that the token is not encrypted, but merely
    /// signed by the [`SecretKey`]. This means that its contents are readable
    /// by everyone.
    ///
    /// If `duration` is `None`, the token will be valid forever. `scope` can be
    /// used to restrict the token to a specific context.
    pub fn token(
        &self,
        duration: Duration,
        issuer: impl ToString,
        subject: impl ToString,
    ) -> Result<Jwt, Error> {
        let claims = Claims::create(duration.into())
            .with_issuer(issuer)
            .with_subject(subject)
            .with_audience(AUDIENCE);

        let token = self
            .secret_key
            .authenticate(claims)
            .map_err(Error::InvalidToken)?;
        Ok(Jwt(token))
    }

    /// Checks if a provided `token` is valid for a given `scope`.
    pub fn verify(&self, token: &Jwt) -> Result<JwtClaims, Error> {
        let mut options = VerificationOptions::default();
        options
            .allowed_audiences
            .get_or_insert(HashSet::new())
            .insert(AUDIENCE.to_owned());

        let claims = self
            .secret_key
            .verify_token(&token.0, Some(options))
            .map_err(Error::InvalidToken)?;

        Ok(claims)
    }
}
