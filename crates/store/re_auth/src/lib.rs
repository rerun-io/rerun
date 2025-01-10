pub use error::Error;
pub use scope::Permission;
pub use service::*;

mod error;
mod scope;
mod service;

use std::{collections::HashSet, time::Duration};

use base64::{engine::general_purpose, Engine as _};
use jwt_simple::{
    claims::{Claims, JWTClaims},
    common::VerificationOptions,
    prelude::{HS256Key, MACLike as _},
    token::Token,
};

/// A common secret that is shared between the client and the server.
///
/// This represents a symmetric authentication scheme, which means that the
/// same key is used to both sign and verify the token.
/// In the future, we will need to support asymmetric schemes too.
///
/// The key is stored unencrypted in memory.
#[derive(Clone)]
#[repr(transparent)]
pub struct SecretKey(HS256Key);

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SecretKey").field(&"********").finish()
    }
}

/// A JWT token that is used to authenticate the client.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Jwt(String);

impl TryFrom<String> for Jwt {
    type Error = Error;
    fn try_from(token: String) -> Result<Self, Self::Error> {
        // We first check if the general structure of the token is correct.
        let _ = Token::decode_metadata(&token).map_err(|_err| Error::MalformedToken)?;
        Ok(Self(token))
    }
}

impl AsRef<str> for Jwt {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl SecretKey {
    /// Generates a new secret key.
    pub fn generate() -> Self {
        let key = HS256Key::generate();
        Self(key)
    }

    /// Restores a secret key from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let key = HS256Key::from_bytes(bytes);
        Self(key)
    }

    /// Encodes the secret key as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes()
    }

    /// Decodes a [`base64`] encoded secret key.
    pub fn from_base64(base64: impl AsRef<str>) -> Result<Self, Error> {
        let bytes = general_purpose::STANDARD
            .decode(base64.as_ref())
            .map_err(Error::InvalidBase64)?;

        let key = HS256Key::from_bytes(&bytes);
        Ok(Self(key))
    }

    /// Encodes the secret key as a [`base64`] string.
    pub fn to_base64(&self) -> String {
        let bytes = self.0.to_bytes();
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
    pub fn token(&self, duration: Option<Duration>, scope: Permission) -> Result<Jwt, Error> {
        let duration = duration.unwrap_or_else(|| Duration::from_secs(u64::MAX));
        let claims = Claims::with_custom_claims(scope, duration.into()).with_audience("rerun");
        let token = self.0.authenticate(claims).map_err(Error::InvalidToken)?;
        Ok(Jwt(token))
    }

    /// Checks if a provided `token` is valid for a given `scope`.
    pub fn verify(&self, token: &Jwt, scope: Permission) -> Result<(), Error> {
        let mut options = VerificationOptions::default();
        options
            .allowed_audiences
            .get_or_insert(HashSet::new())
            .insert("rerun".to_owned());

        let claims: JWTClaims<Permission> = self
            .0
            .verify_token(&token.0, Some(options))
            .map_err(Error::InvalidToken)?;

        scope.allows(claims.custom)
    }
}
