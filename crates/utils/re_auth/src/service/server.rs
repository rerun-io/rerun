use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::{Request, Status};

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};
use crate::provider::VerificationOptions;
use crate::{Error, Jwt, RedapProvider};

#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,

    #[cfg(feature = "oauth")]
    pub permissions: Vec<crate::oauth::Permission>,
}

#[cfg(feature = "oauth")]
impl UserContext {
    pub fn has_read_permission(&self) -> bool {
        use crate::oauth::Permission as P;

        self.permissions
            .iter()
            .any(|p| p == &P::Read || p == &P::ReadWrite)
    }

    pub fn has_write_permission(&self) -> bool {
        use crate::oauth::Permission as P;

        self.permissions.iter().any(|p| p == &P::ReadWrite)
    }
}

impl TryFrom<&MetadataValue<Ascii>> for Jwt {
    type Error = Error;

    fn try_from(value: &MetadataValue<Ascii>) -> Result<Self, Self::Error> {
        let token = value.to_str().map_err(|_err| Error::MalformedToken)?;
        let token = token
            .strip_prefix(TOKEN_PREFIX)
            .ok_or(Error::MalformedToken)?
            .trim();
        Ok(Self(token.to_owned()))
    }
}

/// A basic authenticator that checks for a valid auth token.
#[derive(Clone)]
pub struct Authenticator {
    provider: RedapProvider,
}

impl Authenticator {
    /// Creates a new [`Authenticator`] with the given provider,
    /// which holds the keys used for verification.
    pub fn new(provider: RedapProvider) -> Self {
        Self { provider }
    }
}

impl Interceptor for Authenticator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let mut req = req;

        if let Some(token_metadata) = req.metadata().get(AUTHORIZATION_KEY) {
            let token = Jwt::try_from(token_metadata).map_err(|_err| {
                Status::unauthenticated(crate::ERROR_MESSAGE_MALFORMED_CREDENTIALS)
            })?;

            let claims = self
                .provider
                .verify(&token, VerificationOptions::default())
                .map_err(|_err| {
                    Status::unauthenticated(crate::ERROR_MESSAGE_INVALID_CREDENTIALS)
                })?;

            req.extensions_mut().insert(UserContext {
                user_id: claims.sub().to_owned(),

                #[cfg(feature = "oauth")]
                permissions: claims.permissions().to_vec(),
            });
        }

        Ok(req)
    }
}
