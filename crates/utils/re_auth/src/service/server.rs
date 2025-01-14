use tonic::{
    metadata::{Ascii, MetadataValue},
    service::Interceptor,
    Request, Status,
};

use crate::{provider::VerificationOptions, Error, Jwt, RedapProvider};

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};

#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,
}

impl TryFrom<&MetadataValue<Ascii>> for Jwt {
    type Error = Error;

    fn try_from(value: &MetadataValue<Ascii>) -> Result<Self, Self::Error> {
        let token = value.to_str().map_err(|_err| Error::MalformedToken)?;
        let token = token
            .strip_prefix(TOKEN_PREFIX)
            .ok_or(Error::MalformedToken)?;
        Ok(Self(token.to_owned()))
    }
}

/// A basic authenticator that checks for a valid auth token.
#[derive(Clone)]
pub struct Authenticator {
    secret_key: RedapProvider,
}

impl Authenticator {
    /// Creates a new [`AuthInterceptor`] with the given secret key and scope.
    pub fn new(secret_key: RedapProvider) -> Self {
        Self { secret_key }
    }
}

impl Interceptor for Authenticator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let mut req = req;

        if let Some(token_metadata) = req.metadata().get(AUTHORIZATION_KEY) {
            let token = Jwt::try_from(token_metadata)
                .map_err(|_err| Status::unauthenticated("malformed auth token"))?;

            let claims = self
                .secret_key
                .verify(&token, VerificationOptions::default())
                .map_err(|err| match err {
                    Error::InvalidPermission { .. } => Status::permission_denied(err.to_string()),
                    _ => Status::unauthenticated("invalid credentials"),
                })?;

            req.extensions_mut().insert(UserContext {
                user_id: claims.sub,
            });
        }

        Ok(req)
    }
}
