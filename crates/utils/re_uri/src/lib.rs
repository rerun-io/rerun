//! Rerun uses its own URL scheme to access information across the network.
//!
//! The following schemes are supported: `rerun+http://`, `rerun+https://` and
//! `rerun://`, which is an alias for `rerun+https://`. These schemes are then
//! converted on the fly to either `http://` or `https://`. Rerun uses gRPC-based
//! protocols under the hood, which means that the paths (`/catalog`,
//! `/recording/12345`, …) are mapped to gRPC services and methods on the fly.
//!
//! <div class="warning">
//! In most cases locally running instances of Rerun will not have proper TLS
//! configuration. In these cases, the `rerun+http://` scheme can be used. Naturally,
//! this means that the underlying connection will not be encrypted.
//! </div>
//!
//! The following are examples of valid Rerun URIs:
//!
//! ```
//! for uri in [
//!     // Access the dataplatform catalog.
//!     "rerun://rerun.io",
//!     "rerun://rerun.io:51234/catalog",
//!     "rerun+http://localhost:51234/catalog",
//!     "rerun+https://localhost:51234/catalog",
//!
//!     // Proxy to send messages to another viewer.
//!     "rerun+http://localhost:51234/proxy",
//!
//!     // Links to recording on the dataplatform (optionally with timestamp).
//!     "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@1.23s..72s",
//! ] {
//!     assert!(uri.parse::<re_uri::RedapUri>().is_ok());
//! }
//!
//! ```

mod endpoints;
mod error;
mod fragment;
mod origin;
mod redap_uri;
mod scheme;
mod time_range;

pub use self::{
    endpoints::{catalog::CatalogEndpoint, dataset::DatasetDataEndpoint, proxy::ProxyEndpoint},
    error::Error,
    fragment::Fragment,
    origin::Origin,
    redap_uri::RedapUri,
    scheme::Scheme,
    time_range::TimeRange,
};

pub mod external {
    pub use url;
}
