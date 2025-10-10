//! Official test suite for the Rerun Data Protocol ("redap").
//!
//! ## Usage
//!
//! In the crate containing your implementation of the
//! [`re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService`] trait, add an
//! integration test with the following content:
//!
//! ```ignore
//! async fn build() -> YourRerunCloudServiceImpl {
//!     YourRerunCloudServiceImpl::new()
//! }
//!
//! re_redap_tests::generate_redap_tests!(build);
//! ```

// this is a test suite
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

mod tests;
mod utils;

pub use self::utils::{
    arrow::RecordBatchExt, arrow::SchemaExt, path::TempPath, rerun::TuidPrefix,
    rerun::create_nasty_recording, rerun::create_recording_with_embeddings,
    rerun::create_recording_with_properties, rerun::create_recording_with_scalars,
    rerun::create_recording_with_text,
};

pub use self::tests::*;
