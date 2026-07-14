use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::{Request, Status};

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};
use crate::provider::VerificationOptions;
use crate::{Error, Jwt, Permission, RedapProvider};

#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,

    pub permissions: Vec<Permission>,
}

impl UserContext {
    pub fn has_read_permission(&self) -> bool {
        self.permissions
            .iter()
            .any(|p| p == &Permission::Read || p == &Permission::ReadWrite)
    }

    pub fn has_write_permission(&self) -> bool {
        self.permissions.iter().any(|p| p == &Permission::ReadWrite)
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
    fn call(&mut self, req: Request<()>) -> tonic::Result<Request<()>> {
        let mut req = req;

        if let Some(token_metadata) = req.metadata().get(AUTHORIZATION_KEY) {
            let token = Jwt::try_from(token_metadata).map_err(|_err| {
                Status::unauthenticated(crate::ERROR_MESSAGE_MALFORMED_CREDENTIALS)
            })?;

            let claims = self
                .provider
                .verify(&token, VerificationOptions::default())
                .map_err(|err| {
                    // Log the full error server-side, best-effort
                    // rate-limited to at most once per second to avoid
                    // log storms.
                    static LAST_LOG_MS: AtomicU64 = AtomicU64::new(0);
                    const ONE_SECOND_MS: u64 = 1_000;
                    let now_ms = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis() as u64);
                    if now_ms.saturating_sub(LAST_LOG_MS.load(Ordering::Relaxed)) > ONE_SECOND_MS {
                        LAST_LOG_MS.store(now_ms, Ordering::Relaxed);
                        re_log::warn!("Token verification failed: {err:#}");
                    }

                    // Explicitly provide more detail in the error message, but do not rely
                    // on the error's `Display` implementation, as it may contain sensitive
                    // information.
                    let detail = match err {
                        Error::Jwt(ref jwt_err) => match jwt_err.kind() {
                            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                                "token has expired"
                            }
                            jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                                "invalid token signature"
                            }
                            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm => {
                                "unsupported signature algorithm"
                            }
                            _ => "invalid token",
                        },
                        Error::MalformedToken => "malformed token",
                        _ => "invalid credentials",
                    };
                    Status::unauthenticated(detail)
                })?;

            req.extensions_mut().insert(UserContext {
                user_id: claims.sub().to_owned(),

                permissions: claims.permissions().to_vec(),
            });
        }

        Ok(req)
    }
}
