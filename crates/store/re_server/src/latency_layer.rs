use std::time::Duration;
use tokio::time::sleep;

/// Add artificial latency to each request, to simulate talking to a remote server.
#[derive(Clone)]
pub struct LatencyLayer {
    rtt: Duration,
}

impl LatencyLayer {
    /// Create a new latency layer with the given round-trip time in milliseconds.
    pub fn new(rtt: Duration) -> Self {
        Self { rtt }
    }
}

impl<S> tower::Layer<S> for LatencyLayer {
    type Service = LatencyService<S>;

    fn layer(&self, service: S) -> Self::Service {
        LatencyService {
            inner: service,
            rtt: self.rtt,
        }
    }
}

#[derive(Clone)]
pub struct LatencyService<S> {
    inner: S,
    rtt: Duration,
}

impl<S, Request> tower::Service<Request> for LatencyService<S>
where
    S: tower::Service<Request> + Clone + Send + 'static,
    S::Future: Send,
    S::Response: Send,
    S::Error: Send,
    Request: Send + 'static,
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

    fn call(&mut self, req: Request) -> Self::Future {
        let Self { mut inner, rtt } = self.clone();
        Box::pin(async move {
            let resp = inner.call(req).await;
            sleep(rtt).await;
            resp
        })
    }
}
