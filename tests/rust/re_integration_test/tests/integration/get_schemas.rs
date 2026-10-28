//! Coverage for looking up layer schemas by ID with `/GetSchemas`.

use std::collections::BTreeSet;
use std::str::FromStr as _;

use re_integration_test::{TEST_DATASET_ID, TestServer};
use re_protos::cloud::v1alpha1::ext::{GetSchemasRequest, ScanDatasetManifestDataframe};
use re_protos::common::v1alpha1::ext::SchemaId;
use re_sdk::external::re_log_types::EntryId;
use re_sdk::external::re_tuid::Tuid;

/// More IDs than one request may carry are split across several requests.
#[tokio::test(flavor = "multi_thread")]
async fn get_schemas_splits_ids_over_the_request_limit() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;
    let mut client = server.client().await.expect("Failed to connect to server");
    let dataset_id: EntryId = Tuid::from_str(TEST_DATASET_ID)
        .expect("Failed to parse TUID")
        .into();

    let manifest = client
        .scan_dataset_manifest(
            dataset_id,
            [ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256_NAME],
            None,
            None,
        )
        .await
        .expect("Failed to scan the dataset manifest");
    let registered: BTreeSet<SchemaId> = manifest
        .batches
        .iter()
        .flat_map(|batch| {
            ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256
                .extract(batch)
                .expect("Failed to read the schema hashes")
                .iter_owned()
                .collect::<Vec<_>>()
        })
        .map(|sha256| SchemaId::from_sha256(sha256.expect("registered layers have a schema hash")))
        .collect();

    let unknown = (0..GetSchemasRequest::MAX_SCHEMA_IDS as u32).map(|index| {
        let mut sha256 = [0xff; 32];
        sha256[..4].copy_from_slice(&index.to_le_bytes());
        SchemaId::from_sha256(sha256)
    });
    let requested: BTreeSet<SchemaId> =
        std::iter::chain(registered.iter().copied(), unknown).collect();
    assert!(
        requested.len() > GetSchemasRequest::MAX_SCHEMA_IDS,
        "the test must exceed the per-request limit"
    );

    let schemas = client
        .get_schemas(dataset_id, &requested)
        .await
        .expect("Failed to get schemas");

    assert_eq!(
        schemas.schemas.keys().copied().collect::<BTreeSet<_>>(),
        registered
    );
}
