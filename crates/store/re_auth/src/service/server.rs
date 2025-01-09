use tonic::{
    metadata::{Ascii, MetadataValue},
    service::Interceptor,
    Request, Status,
};

use crate::{Error, Jwt, Scope, SecretKey};

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};

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

/// A basic [`tonic::Interceptor`] that checks for a valid auth token.
pub struct AuthInterceptor {
    secret_key: SecretKey,
    scope: Scope,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let Some(token_metadata) = req.metadata().get(AUTHORIZATION_KEY) else {
            return Err(Status::unauthenticated("missing valid auth token"));
        };

        let token = Jwt::try_from(token_metadata)
            .map_err(|_err| Status::unauthenticated("malformed auth token"))?;

        self.secret_key
            .verify(&token, self.scope)
            .map_err(|err| match err {
                Error::InvalidPermission { .. } => Status::permission_denied(err.to_string()),
                _ => Status::unauthenticated("invalid token"),
            })?;

        Ok(req)
    }
}
