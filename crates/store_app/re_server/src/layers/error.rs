use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;

/// Shared state for injecting errors into specific gRPC endpoints.
///
/// Holds a set of gRPC method names (e.g. `"FetchChunks"`) that should
/// fail with a `NotFound` error. Used for testing error propagation.
#[derive(Clone)]
pub struct InjectedErrors(Arc<Mutex<HashSet<String>>>);

impl Default for InjectedErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl InjectedErrors {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashSet::new())))
    }

    /// Mark a gRPC endpoint to fail. The `method` is matched against the
    /// gRPC method name, e.g. `"FetchChunks"`.
    pub fn inject(&self, method: &str) {
        self.0.lock().insert(method.to_owned());
    }

    /// Stop failing a previously injected endpoint.
    pub fn clear(&self, method: &str) {
        self.0.lock().remove(method);
    }

    /// Stop failing all endpoints.
    pub fn clear_all(&self) {
        self.0.lock().clear();
    }

    /// Check if the given URI path should fail.
    ///
    /// Extracts the method name (last `/`-separated segment) from the path
    /// and checks it against the set.
    fn check_path(&self, path: &str) -> Option<String> {
        let method = path.rsplit('/').next().unwrap_or(path);
        let set = self.0.lock();
        if set.contains(method) {
            Some(method.to_owned())
        } else {
            None
        }
    }
}

// --- Tower layer ---

/// A tower [`tower::Layer`] that rejects requests to gRPC endpoints registered in [`InjectedErrors`].
///
/// When a request's URI path ends with a registered method name, the layer
/// short-circuits with a `tonic::Status::not_found` error without calling
/// the inner service. This works for any gRPC endpoint.
#[derive(Clone)]
pub struct ErrorInjectionLayer {
    errors: InjectedErrors,
}

impl ErrorInjectionLayer {
    pub fn new(errors: InjectedErrors) -> Self {
        Self { errors }
    }
}

impl<S> tower::Layer<S> for ErrorInjectionLayer {
    type Service = ErrorInjectionService<S>;

    fn layer(&self, service: S) -> Self::Service {
        ErrorInjectionService {
            inner: service,
            errors: self.errors.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ErrorInjectionService<S> {
    inner: S,
    errors: InjectedErrors,
}

impl<S, ReqBody, ResBody> tower::Service<http::Request<ReqBody>> for ErrorInjectionService<S>
where
    S: tower::Service<http::Request<ReqBody>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    S::Error: Send,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        if let Some(method) = self.errors.check_path(req.uri().path()) {
            let status = tonic::Status::not_found(format!(
                "injected error for testing: {method} deliberately failed"
            ));
            // `tonic::Status::into_http` produces a valid gRPC error response
            // with `grpc-status` and `grpc-message` headers and a default (empty) body.
            return Box::pin(async move { Ok(status.into_http()) });
        }

        // See: https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        Box::pin(async move { inner.call(req).await })
    }
}
