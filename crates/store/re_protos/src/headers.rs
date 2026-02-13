/// The HTTP header key to pass an entry ID to the `RerunCloudService` APIs.
pub const RERUN_HTTP_HEADER_ENTRY_ID: &str = "x-rerun-entry-id";

/// The HTTP header key to pass an entry name to the `RerunCloudService` APIs.
///
/// This will automatically be resolved to an entry ID, as long as a dataset with the associated
/// name can be found in the database.
///
/// This is serialized as base64-encoded data (hence `-bin`), since entry names can be any UTF8 strings,
/// while HTTP2 headers only support ASCII.
pub const RERUN_HTTP_HEADER_ENTRY_NAME: &str = "x-rerun-entry-name-bin";

/// The HTTP header key that all our official gRPC clients use to specify their identity and version.
///
/// All our official gRPC servers make sure to always return a copy of this header to the client as-is, in
/// addition to propagating it into our gRPC metrics, traces and metrics.
pub const RERUN_HTTP_HEADER_CLIENT_VERSION: &str = "x-rerun-client-version";

/// The HTTP header key that all our official gRPC servers use to specify their identity and version.
///
/// All our official gRPC servers always set this header in all their responses, in addition to
/// propagating it into our gRPC metrics, traces and metrics.
pub const RERUN_HTTP_HEADER_SERVER_VERSION: &str = "x-rerun-server-version";

/// HTTP authorization header key, used to transport authorization tokens
pub const HTTP_HEADER_AUTHORIZATION: &str = "authorization";

/// Extension trait for [`tonic::Request`] to inject Rerun Data Protocol headers into gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersInjectorExt as _;
/// let mut req = tonic::Request::new(()).with_entry_name("droid:sample2k").unwrap();
/// ```
pub trait RerunHeadersInjectorExt: Sized {
    fn with_entry_id(self, entry_id: re_log_types::EntryId) -> tonic::Result<Self>;

    fn with_entry_name(self, entry_name: impl AsRef<str>) -> tonic::Result<Self>;

    fn with_metadata(self, md: &tonic::metadata::MetadataMap) -> Self;
}

impl<T> RerunHeadersInjectorExt for tonic::Request<T> {
    fn with_entry_id(mut self, entry_id: re_log_types::EntryId) -> tonic::Result<Self> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_ID;

        let entry_id = entry_id.to_string();
        let entry_id = entry_id.parse().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        self.metadata_mut().insert(HEADER, entry_id);

        Ok(self)
    }

    fn with_entry_name(mut self, entry_name: impl AsRef<str>) -> tonic::Result<Self> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_NAME;

        let entry_name = entry_name.as_ref();
        let entry_name = tonic::metadata::BinaryMetadataValue::from_bytes(entry_name.as_bytes());

        self.metadata_mut().insert_bin(HEADER, entry_name);

        Ok(self)
    }

    fn with_metadata(mut self, md: &tonic::metadata::MetadataMap) -> Self {
        if let Some(entry_id) = md.get(RERUN_HTTP_HEADER_ENTRY_ID).cloned() {
            self.metadata_mut()
                .insert(RERUN_HTTP_HEADER_ENTRY_ID, entry_id);
        }

        if let Some(entry_name) = md.get_bin(RERUN_HTTP_HEADER_ENTRY_NAME).cloned() {
            self.metadata_mut()
                .insert_bin(RERUN_HTTP_HEADER_ENTRY_NAME, entry_name);
        }

        if let Some(auth) = md.get(HTTP_HEADER_AUTHORIZATION).cloned() {
            self.metadata_mut().insert(HTTP_HEADER_AUTHORIZATION, auth);
        }

        self
    }
}

/// Extension trait for [`tonic::Request`] to extract Rerun Data Protocol headers from gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersExtractorExt as _;
/// # let req = tonic::Request::new(());
/// let entry_id = req.entry_id().unwrap();
/// ```
pub trait RerunHeadersExtractorExt {
    fn entry_id(&self) -> tonic::Result<Option<re_log_types::EntryId>>;

    fn entry_name(&self) -> tonic::Result<Option<String>>;
}

impl<T> RerunHeadersExtractorExt for tonic::Request<T> {
    fn entry_id(&self) -> tonic::Result<Option<re_log_types::EntryId>> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_ID;

        let Some(entry_id) = self.metadata().get(HEADER) else {
            return Ok(None);
        };

        let entry_id = entry_id.to_str().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;
        let entry_id = entry_id.parse().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        Ok(Some(entry_id))
    }

    fn entry_name(&self) -> tonic::Result<Option<String>> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_NAME;

        let Some(entry_name) = self.metadata().get_bin(HEADER) else {
            return Ok(None);
        };

        let entry_name = entry_name.to_bytes().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_name:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;
        let entry_name = String::from_utf8(entry_name.to_vec()).map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_name:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        Ok(Some(entry_name))
    }
}

// ---

pub type RerunHeadersLayer = tower::layer::util::Stack<
    PropagateHeadersLayer,
    tower::layer::util::Stack<
        tonic::service::InterceptorLayer<RerunVersionInterceptor>,
        tower::layer::util::Identity,
    >,
>;

/// Instantiates a compound [`tower::Layer`] that handles all things related to Rerun headers.
pub fn new_rerun_headers_layer(
    name: Option<String>,
    version: Option<String>,
    is_client: bool,
) -> RerunHeadersLayer {
    tower::ServiceBuilder::new()
        .layer(tonic::service::interceptor::InterceptorLayer::new({
            RerunVersionInterceptor::new(is_client, name, version)
        }))
        .layer(new_rerun_headers_propagation_layer())
        .into_inner()
}

/// Creates a new [`tower::Layer`] middleware that always makes sure to propagate Rerun headers
/// back and forth across requests and responses.
pub fn new_rerun_headers_propagation_layer() -> PropagateHeadersLayer {
    PropagateHeadersLayer::new(
        [
            http::HeaderName::from_static(RERUN_HTTP_HEADER_ENTRY_ID),
            http::HeaderName::from_static(RERUN_HTTP_HEADER_CLIENT_VERSION),
            http::HeaderName::from_static(RERUN_HTTP_HEADER_SERVER_VERSION),
        ]
        .into_iter()
        .collect(),
    )
}

/// Implements a `[tonic::service::Interceptor]` that records the identity and version of the client and/or server
/// in well-known headers.
///
/// See also [`RERUN_HTTP_HEADER_CLIENT_VERSION`] & [`RERUN_HTTP_HEADER_SERVER_VERSION`].
#[derive(Clone)]
pub struct RerunVersionInterceptor {
    is_client: bool,
    name: String,
    version: String,
}

impl RerunVersionInterceptor {
    pub fn new_client(name: Option<String>, version: Option<String>) -> Self {
        Self::new(true, name, version)
    }

    pub fn new_server(name: Option<String>, version: Option<String>) -> Self {
        Self::new(false, name, version)
    }

    pub fn new(is_client: bool, name: Option<String>, version: Option<String>) -> Self {
        let mut name = name
            .or_else(|| std::env::var("OTEL_SERVICE_NAME").ok())
            .or_else(|| {
                let path = std::env::current_exe().ok()?;
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_owned());

        if !name.is_ascii() {
            // Cannot have non ASCII data in HTTP headers.
            name = "<non_ascii_name_redacted>".to_owned();
        }

        let version = version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());

        Self {
            is_client,
            name,
            version,
        }
    }
}

impl tonic::service::Interceptor for RerunVersionInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> tonic::Result<tonic::Request<()>> {
        let Self {
            is_client,
            name,
            version,
        } = self;

        let version = format!("{name}/{version}");

        req.metadata_mut().insert(
            if *is_client {
                RERUN_HTTP_HEADER_CLIENT_VERSION
            } else {
                RERUN_HTTP_HEADER_SERVER_VERSION
            },
            version
                .parse()
                .expect("cannot fail, checked in constructor"),
        );

        Ok(req)
    }
}

// ---

// NOTE: This if a fork of <https://docs.rs/tower-http/0.6.6/tower_http/propagate_header/struct.PropagateHeader.html>.
//
// It exists to prevent never-ending chains of generics when propagating multiple headers, e.g.:
// ```
// pub type RedapClientInner =
//     re_perf_telemetry::external::tower_http::propagate_header::PropagateHeader<
//         re_perf_telemetry::external::tower_http::propagate_header::PropagateHeader<
//             re_perf_telemetry::external::tower_http::propagate_header::PropagateHeader<
//                 re_perf_telemetry::external::tower_http::propagate_header::PropagateHeader<
//                     re_perf_telemetry::external::tower_http::trace::Trace<
//                         tonic::service::interceptor::InterceptedService<
//                             tonic::service::interceptor::InterceptedService<
//                                 tonic::transport::Channel,
//                                 re_auth::client::AuthDecorator,
//                             >,
//                             re_perf_telemetry::TracingInjectorInterceptor,
//                         >,
//                         re_perf_telemetry::external::tower_http::classify::SharedClassifier<
//                             re_perf_telemetry::external::tower_http::classify::GrpcErrorsAsFailures,
//                         >,
//                         re_perf_telemetry::GrpcMakeSpan,
//                     >,
//                 >,
//             >,
//         >,
//     >;
// ```
// which instead becomes this:
// ```
// pub type RedapClientInner =
//     PropagateHeaders<
//         re_perf_telemetry::external::tower_http::trace::Trace<
//             tonic::service::interceptor::InterceptedService<
//                 tonic::service::interceptor::InterceptedService<
//                     tonic::transport::Channel,
//                     re_auth::client::AuthDecorator,
//                 >,
//                 re_perf_telemetry::TracingInjectorInterceptor,
//             >,
//             re_perf_telemetry::external::tower_http::classify::SharedClassifier<
//                 re_perf_telemetry::external::tower_http::classify::GrpcErrorsAsFailures,
//             >,
//             re_perf_telemetry::GrpcMakeSpan,
//         >,
//     >;
// ```

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use http::header::HeaderName;
use http::{HeaderValue, Request, Response};
use pin_project_lite::pin_project;
use tower::Service;
use tower::layer::Layer;

/// Layer that applies [`PropagateHeaders`] which propagates multiple headers at once from requests to responses.
///
/// If the headers are present on the request they'll be applied to the response as well. This could
/// for example be used to propagate headers such as `x-rerun-entry-id`, `x-rerun-client-version`, etc.
#[derive(Clone, Debug)]
pub struct PropagateHeadersLayer {
    headers: HashSet<HeaderName>,
}

impl PropagateHeadersLayer {
    /// Create a new [`PropagateHeadersLayer`].
    pub fn new(headers: HashSet<HeaderName>) -> Self {
        Self { headers }
    }
}

impl<S> Layer<S> for PropagateHeadersLayer {
    type Service = PropagateHeaders<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PropagateHeaders {
            inner,
            headers: self.headers.clone(),
        }
    }
}

/// Middleware that propagates multiple headers at once from requests to responses.
///
/// If the headers are present on the request they'll be applied to the response as well. This could
/// for example be used to propagate headers such as `x-rerun-entry-id`, `x-rerun-client-version`, etc.
#[derive(Clone, Debug)]
pub struct PropagateHeaders<S> {
    inner: S,
    headers: HashSet<HeaderName>,
}

impl<S> PropagateHeaders<S> {
    /// Create a new [`PropagateHeaders`] that propagates the given header.
    pub fn new(inner: S, headers: HashSet<HeaderName>) -> Self {
        Self { inner, headers }
    }

    /// Returns a new [`Layer`] that wraps services with a `PropagateHeaders` middleware.
    ///
    /// [`Layer`]: tower::layer::Layer
    pub fn layer(headers: HashSet<HeaderName>) -> PropagateHeadersLayer {
        PropagateHeadersLayer::new(headers)
    }
}

impl<ReqBody, ResBody, S> Service<Request<ReqBody>> for PropagateHeaders<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let headers_and_values = self
            .headers
            .iter()
            .filter_map(|name| {
                req.headers()
                    .get(name)
                    .cloned()
                    .map(|value| (name.clone(), value))
            })
            .collect();

        ResponseFuture {
            future: self.inner.call(req),
            headers_and_values,
        }
    }
}

pin_project! {
    /// Response future for [`PropagateHeaders`].
    #[derive(Debug)]
    pub struct ResponseFuture<F> {
        #[pin]
        future: F,
        headers_and_values: Vec<(HeaderName, HeaderValue)>,
    }
}

impl<F, ResBody, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut res = ready!(this.future.poll(cx)?);

        for (header, value) in std::mem::take(this.headers_and_values) {
            res.headers_mut().insert(header, value);
        }

        Poll::Ready(Ok(res))
    }
}
