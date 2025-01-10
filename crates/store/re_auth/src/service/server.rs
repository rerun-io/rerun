use tonic::{
    metadata::{Ascii, MetadataValue},
    Request, Status,
};

use crate::{Error, Jwt, Permission, SecretKey};

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

/// A basic authenticator that checks for a valid auth token.
#[derive(Clone)]
pub struct Authenticator {
    secret_key: SecretKey,
}

impl Authenticator {
    /// Creates a new [`AuthInterceptor`] with the given secret key and scope.
    pub fn new(secret_key: SecretKey) -> Self {
        Self { secret_key }
    }
}

impl Authenticator {
    /// Checks if the request has a valid auth token that also contains the correct permission.
    pub fn verify<T>(&self, permission: Permission, req: Request<T>) -> Result<Request<T>, Status> {
        let Some(token_metadata) = req.metadata().get(AUTHORIZATION_KEY) else {
            return Err(Status::unauthenticated("missing valid auth token"));
        };

        let token = Jwt::try_from(token_metadata)
            .map_err(|_err| Status::unauthenticated("malformed auth token"))?;

        self.secret_key
            .verify(&token, permission)
            .map_err(|err| match err {
                Error::InvalidPermission { .. } => Status::permission_denied(err.to_string()),
                _ => Status::unauthenticated("invalid token"),
            })?;

        Ok(req)
    }
}
