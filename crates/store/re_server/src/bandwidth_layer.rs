use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Buf;
use http_body::Frame;

/// Tower layer that rate-limits response body delivery to simulate limited bandwidth.
#[derive(Clone)]
pub struct BandwidthLayer {
    bytes_per_second: Option<u64>,
}

impl BandwidthLayer {
    pub fn new(bytes_per_second: Option<u64>) -> Self {
        Self { bytes_per_second }
    }
}

impl<S> tower::Layer<S> for BandwidthLayer {
    type Service = BandwidthService<S>;

    fn layer(&self, service: S) -> Self::Service {
        BandwidthService {
            inner: service,
            bytes_per_second: self.bytes_per_second,
        }
    }
}

#[derive(Clone)]
pub struct BandwidthService<S> {
    inner: S,
    bytes_per_second: Option<u64>,
}

impl<S, ReqBody, ResBody> tower::Service<http::Request<ReqBody>> for BandwidthService<S>
where
    S: tower::Service<http::Request<ReqBody>, Response = http::Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    S::Response: Send,
    S::Error: Send,
    ReqBody: Send + 'static,
    ResBody: http_body::Body + Send + 'static,
    ResBody::Data: Send,
    ResBody::Error: Send,
{
    type Response = http::Response<BandwidthBody<ResBody>>;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        let bytes_per_second = self.bytes_per_second;
        Box::pin(async move {
            let resp = inner.call(req).await?;
            let (parts, body) = resp.into_parts();
            let body = BandwidthBody::new(body, bytes_per_second);
            Ok(http::Response::from_parts(parts, body))
        })
    }
}

/// Wraps an [`http_body::Body`] and rate-limits frame delivery.
pub struct BandwidthBody<B> {
    inner: Pin<Box<B>>,
    bytes_per_second: Option<u64>,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl<B: Default> Default for BandwidthBody<B> {
    fn default() -> Self {
        Self::new(B::default(), None)
    }
}

impl<B> BandwidthBody<B> {
    fn new(body: B, bytes_per_second: Option<u64>) -> Self {
        Self {
            inner: Box::pin(body),
            bytes_per_second,
            sleep: None,
        }
    }
}

impl<B> http_body::Body for BandwidthBody<B>
where
    B: http_body::Body + Send,
    B::Data: Buf,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();

        // If we have a pending sleep from a previous frame, wait for it first.
        if let Some(sleep) = &mut this.sleep {
            match sleep.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(()) => {
                    this.sleep = None;
                }
            }
        }

        // Poll the inner body for the next frame.
        let poll = this.inner.as_mut().poll_frame(cx);

        if let Poll::Ready(Some(Ok(ref frame))) = poll
            && let Some(bytes_per_second) = this.bytes_per_second
            && let Some(data) = frame.data_ref()
        {
            let frame_bytes = data.remaining() as u64;
            if frame_bytes > 0 {
                let secs = frame_bytes as f64 / bytes_per_second as f64;
                if let Ok(delay) = std::time::Duration::try_from_secs_f64(secs) {
                    this.sleep = Some(Box::pin(tokio::time::sleep(delay)));
                }
            }
        }

        poll
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}
