//! Tests for the `rerun_storage_url` column of the dataset manifest, and for the `memory://` URLs
//! it reports for layers written straight into the server.

#![cfg(feature = "lance")]
#![expect(clippy::unwrap_used)]

use futures::TryStreamExt as _;
use itertools::Itertools as _;

use re_protos::cloud::v1alpha1::ScanDatasetManifestRequest;
use re_protos::cloud::v1alpha1::ext;
use re_protos::cloud::v1alpha1::ext::ScanDatasetManifestDataframe;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService as _;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_tests::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};
use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

/// A layer registered from a file reports the `file://` URI it was registered from, which is what
/// the viewer shows as an asset's source. A file with a footer is read on demand rather than
/// loaded into memory, so the URI is also where the layer's chunks still live.
#[tokio::test]
async fn file_registration_reports_its_file_url() {
    let service = RerunCloudHandlerBuilder::new().build();

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("segment1", &["my/entity"])],
    );

    service.create_dataset_entry_with_name("my_dataset").await;
    service
        .register_with_dataset_name_blocking("my_dataset", data_sources_def.to_data_sources())
        .await;

    let registered_url = data_sources_def.to_data_sources_ext()[0]
        .storage_url
        .to_string();
    assert!(
        registered_url.starts_with("file://"),
        "the test should register from a file, got: {registered_url}"
    );

    let manifest = scan_manifest(&service, "my_dataset").await;
    let urls = ScanDatasetManifestDataframe::COLUMN_RERUN_STORAGE_URL
        .extract(&manifest)
        .unwrap();

    assert_eq!(urls.value_owned(0), registered_url);
}

/// Registering a `memory://` URL that was never registered returns `NOT_FOUND`.
#[tokio::test]
async fn register_memory_url_not_found() {
    let service = RerunCloudHandlerBuilder::new().build();

    service.create_dataset_entry_with_name("dataset_nf").await;

    let fake_memory_url = format!("memory:///store/{}", re_tuid::Tuid::new());

    let memory_data_source: re_protos::cloud::v1alpha1::DataSource =
        ext::DataSource::new_rrd(&fake_memory_url).unwrap().into();

    let request = tonic::Request::new(re_protos::cloud::v1alpha1::RegisterWithDatasetRequest {
        data_sources: vec![memory_data_source],
        on_duplicate: Default::default(),
    })
    .with_entry_name(entry_name("dataset_nf"));

    let result = service.register_with_dataset(request).await;
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::NotFound,
        "should get NOT_FOUND for an unknown memory URL"
    );
}

// --- helpers ---

async fn scan_manifest(
    service: &RerunCloudHandler,
    dataset_name: &str,
) -> arrow::array::RecordBatch {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest::all())
                .with_entry_name(entry_name(dataset_name)),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches: Vec<arrow::array::RecordBatch> = responses
        .into_iter()
        .map(|resp| resp.data.unwrap().try_into().unwrap())
        .collect_vec();

    arrow::compute::concat_batches(
        batches
            .first()
            .expect("there should be at least one batch")
            .schema_ref(),
        &batches,
    )
    .unwrap()
}
