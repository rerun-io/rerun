//! Tests for `memory://` URL re-registration and pool cleanup.
//!
//! These live in `re_server` (not `re_redap_tests`) because `memory://` URLs are
//! OSS-specific — the cloud implementation doesn't use them.

#![cfg(feature = "lance")]
#![expect(clippy::unwrap_used)]

use arrow::array::StringArray;
use futures::TryStreamExt as _;
use itertools::Itertools as _;

use re_protos::cloud::v1alpha1::ScanDatasetManifestRequest;
use re_protos::cloud::v1alpha1::ext;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService as _;
use re_protos::cloud::v1alpha1::{DeleteEntryRequest, ScanDatasetManifestResponse};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_tests::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, register_and_wait,
};
use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

fn build() -> RerunCloudHandler {
    RerunCloudHandlerBuilder::new().build()
}

/// Test the cross-dataset memory:// re-registration flow:
/// 1. Register an RRD to dataset A, obtain its memory:// URL
/// 2. Re-register that memory:// URL to dataset B
/// 3. Verify dataset B can see the store
/// 4. Delete dataset A, verify dataset B still has access (strong ref)
/// 5. Delete dataset B, then attempt re-register to dataset C → `NOT_FOUND`
#[tokio::test]
async fn register_memory_url_cross_dataset() {
    let service = build();

    // --- Step 1: Create dataset A and register an RRD ---
    let dataset_a = service.create_dataset_entry_with_name("dataset_a").await;

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::simple("segment1", &["my/entity"])],
    );

    service
        .register_with_dataset_name_blocking("dataset_a", data_sources_def.to_data_sources())
        .await;

    // Extract the memory:// URL from the manifest
    let manifest_a = scan_manifest(&service, "dataset_a").await;
    let urls = manifest_a
        .column_by_name(ScanDatasetManifestResponse::FIELD_STORAGE_URL)
        .unwrap()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let memory_url = urls.value(0).to_owned();
    assert!(
        memory_url.starts_with("memory:///store/"),
        "expected memory URL, got: {memory_url}"
    );

    // --- Step 2: Create dataset B, register using the memory:// URL ---
    let dataset_b = service.create_dataset_entry_with_name("dataset_b").await;

    let memory_data_source: re_protos::cloud::v1alpha1::DataSource = ext::DataSource {
        storage_url: url::Url::parse(&memory_url).unwrap(),
        is_prefix: false,
        layer: ext::DataSource::DEFAULT_LAYER.to_owned(),
        kind: ext::DataSourceKind::Rrd,
    }
    .into();

    let request = tonic::Request::new(re_protos::cloud::v1alpha1::RegisterWithDatasetRequest {
        data_sources: vec![memory_data_source.clone()],
        on_duplicate: Default::default(),
    })
    .with_entry_name("dataset_b")
    .unwrap();

    let task_results = register_and_wait(&service, request).await;
    assert!(
        !task_results.is_empty(),
        "registering memory URL should produce task results"
    );

    // --- Step 3: Scan dataset B's manifest → assert 1 row ---
    let manifest_b = scan_manifest(&service, "dataset_b").await;
    assert_eq!(
        manifest_b.num_rows(),
        1,
        "dataset B manifest should have 1 row after re-registration"
    );

    // --- Step 4: Delete dataset A → scan dataset B again → still 1 row ---
    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(dataset_a.details.id.into()),
        }))
        .await
        .expect("delete dataset A should succeed");

    let manifest_b_after_delete = scan_manifest(&service, "dataset_b").await;
    assert_eq!(
        manifest_b_after_delete.num_rows(),
        1,
        "dataset B manifest should still have 1 row after deleting dataset A (B holds the strong ref)"
    );

    // --- Step 5: Delete dataset B, then attempt re-register with same URL → NOT_FOUND ---
    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(dataset_b.details.id.into()),
        }))
        .await
        .expect("delete dataset B should succeed");

    let _dataset_c = service.create_dataset_entry_with_name("dataset_c").await;

    let request = tonic::Request::new(re_protos::cloud::v1alpha1::RegisterWithDatasetRequest {
        data_sources: vec![memory_data_source],
        on_duplicate: Default::default(),
    })
    .with_entry_name("dataset_c")
    .unwrap();

    let result = service.register_with_dataset(request).await;
    assert!(
        result.is_err(),
        "re-registration should fail after all datasets holding the store are deleted"
    );
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::NotFound,
        "should get NOT_FOUND for a memory URL whose store has been dropped"
    );
}

/// Test that registering a memory:// URL that was never registered returns `NOT_FOUND`.
#[tokio::test]
async fn register_memory_url_not_found() {
    let service = build();

    service.create_dataset_entry_with_name("dataset_nf").await;

    // Construct a memory:// URL with a random Tuid that was never registered
    let fake_tuid = re_tuid::Tuid::new();
    let fake_memory_url = format!("memory:///store/{fake_tuid}");

    let memory_data_source: re_protos::cloud::v1alpha1::DataSource = ext::DataSource {
        storage_url: url::Url::parse(&fake_memory_url).unwrap(),
        is_prefix: false,
        layer: ext::DataSource::DEFAULT_LAYER.to_owned(),
        kind: ext::DataSourceKind::Rrd,
    }
    .into();

    let request = tonic::Request::new(re_protos::cloud::v1alpha1::RegisterWithDatasetRequest {
        data_sources: vec![memory_data_source],
        on_duplicate: Default::default(),
    })
    .with_entry_name("dataset_nf")
    .unwrap();

    let result = service.register_with_dataset(request).await;
    assert!(
        result.is_err(),
        "registering a never-registered memory URL should fail"
    );
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::NotFound,
        "should get NOT_FOUND for an unknown memory URL"
    );
}

// --- helpers ---

async fn scan_manifest(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    dataset_name: &str,
) -> arrow::array::RecordBatch {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest { columns: vec![] })
                .with_entry_name(dataset_name)
                .unwrap(),
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
