use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::header::InvalidHeaderValue;
use tower::{Layer, Service};

use super::{AUTHORIZATION_KEY, TOKEN_PREFIX};
use crate::Jwt;
use crate::credentials::{CredentialsProvider, StaticCredentialsProvider};

/// Client-side async auth layer (replaces `Interceptor`)
#[derive(Clone)]
pub struct AuthDecorator {
    provider: Option<Arc<dyn CredentialsProvider + Send + Sync>>,
}

impl AuthDecorator {
    pub fn new(provider: Option<Arc<dyn CredentialsProvider + Send + Sync>>) -> Self {
        Self { provider }
    }

    pub fn from_token(token: Jwt) -> Self {
        Self {
            provider: Some(Arc::new(StaticCredentialsProvider::new(token))),
        }
    }
}

impl<S> Layer<S> for AuthDecorator {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            provider: self.provider.clone(),
            inner,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthService<S> {
    provider: Option<Arc<dyn CredentialsProvider + Send + Sync>>,
    inner: S,
}

type BoxFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for AuthService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: crate::wasm_compat::SendIfNotWasm + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|err| err.into())
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        // See: https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let provider = self.provider.clone();

        Box::pin(async move {
            let mut req = req;

            if let Some(provider) = provider {
                match provider.get_token().await {
                    Ok(Some(jwt)) => {
                        let token = jwt.0.trim();

                        match format!("{TOKEN_PREFIX}{token}")
                            .parse::<http::HeaderValue>()
                            .map_err(|err: InvalidHeaderValue| {
                                re_log::debug!("malformed token '{token}': {err}");
                                err
                            }) {
                            Ok(token) => {
                                req.headers_mut().insert(AUTHORIZATION_KEY, token);

                                crate::wasm_compat::make_future_send_on_wasm(inner.call(req))
                                    .await
                                    .map_err(|err| err.into())
                            }
                            Err(err) => Err(err.into()),
                        }
                    }

                    // will probably turn into a 403
                    Ok(None) => crate::wasm_compat::make_future_send_on_wasm(inner.call(req))
                        .await
                        .map_err(|err| err.into()),

                    Err(err) => Err(err.into()),
                }
            } else {
                crate::wasm_compat::make_future_send_on_wasm(inner.call(req))
                    .await
                    .map_err(|err| err.into())
            }
        })
    }
}
