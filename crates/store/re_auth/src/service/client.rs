use re_log::error;

use tonic::{metadata::errors::InvalidMetadataValue, service::Interceptor, Request, Status};

use crate::Jwt;

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};

pub struct AuthDecorator {
    token: Jwt,
}

impl Interceptor for AuthDecorator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        let token = format!("{TOKEN_PREFIX}{}", self.token.as_ref())
            .parse()
            .map_err(|err: InvalidMetadataValue| {
                error!("malformed token: {}", err.to_string());
                Status::invalid_argument("malformed token")
            })?;

        let mut req = req;
        req.metadata_mut().insert(AUTHORIZATION_KEY, token);

        Ok(req)
    }
}
