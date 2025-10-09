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

// use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
//
// pub async fn list_entries_table<T: RerunCloudService>(builder: impl FnOnce() -> T) {
//     tests::entries_table::list_entries_table(builder()).await;
// }
//
// #[macro_export]
// macro_rules! generate_redap_tests {
//     ($builder:ident) => {
//         #[tokio::test]
//         async fn list_entries_table() {
//             ::re_redap_tests::list_entries_table(|| $builder()).await
//         }
//     };
// }
