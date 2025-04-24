use re_log::error;

use tonic::{metadata::errors::InvalidMetadataValue, service::Interceptor, Request, Status};

use crate::Jwt;

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};

#[derive(Default, Clone)]
pub struct AuthDecorator {
    jwt: Option<Jwt>,
}

impl AuthDecorator {
    pub fn new(jwt: Option<Jwt>) -> Self {
        Self { jwt }
    }
}

impl Interceptor for AuthDecorator {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        if let Some(jwt) = self.jwt.as_ref() {
            let token = format!("{TOKEN_PREFIX}{}", jwt.0).parse().map_err(
                |err: InvalidMetadataValue| {
                    error!("malformed token: {}", err.to_string());
                    Status::invalid_argument("malformed token")
                },
            )?;

            let mut req = req;
            req.metadata_mut().insert(AUTHORIZATION_KEY, token);

            Ok(req)
        } else {
            Ok(req)
        }
    }
}
