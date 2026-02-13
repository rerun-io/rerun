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
#![expect(clippy::unwrap_used, clippy::disallowed_methods)]

mod tests;
mod utils;

pub use ::re_arrow_util::{FieldsTestExt, RecordBatchTestExt, SchemaTestExt};

pub use self::tests::*;
pub use self::utils::path::TempPath;
pub use self::utils::rerun::{
    TuidPrefix, create_minimal_binary_recording_in, create_nasty_recording,
    create_recording_with_embeddings, create_recording_with_properties,
    create_recording_with_scalars, create_recording_with_static_components,
    create_recording_with_text, create_simple_blueprint, create_simple_recording,
    create_simple_recording_in,
}; // convenience
