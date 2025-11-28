use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use jsonwebtoken::decode_header;

use crate::oauth::{MalformedTokenError, RerunCloudClaims};

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("token does not seem to be a valid JWT token: {0}")]
    MalformedToken(#[source] jsonwebtoken::errors::Error),
}

/// A JWT that is used to authenticate the client.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Jwt(pub(crate) String);

impl Jwt {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn unverified_claims(&self) -> Result<RerunCloudClaims, MalformedTokenError> {
        let (_header, rest) = self
            .as_str()
            .split_once('.')
            .ok_or(MalformedTokenError::MissingHeaderPayloadSeparator)?;
        let (payload, _signature) = rest
            .split_once('.')
            .ok_or(MalformedTokenError::MissingPayloadSignatureSeparator)?;
        let payload = BASE64_URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(MalformedTokenError::Base64)?;
        let claims: RerunCloudClaims =
            serde_json::from_slice(&payload).map_err(MalformedTokenError::Serde)?;
        Ok(claims)
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
