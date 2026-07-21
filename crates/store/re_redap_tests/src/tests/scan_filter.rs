//! End-to-end coverage for the server-side `filter` field on `ScanSegmentTable` and
//! `ScanDatasetManifest`. Runs against both the OSS server (`re_server`) and the cloud frontend.

use arrow::array::{RecordBatch, StringArray};
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{ScanDatasetManifestRequest, ScanSegmentTableRequest};
use re_protos::headers::RerunHeadersInjectorExt as _;

use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};

const SEGMENT_ID_COL: &str = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;

async fn setup(service: &impl RerunCloudService, dataset_name: &str) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities("my_segment_id1", &["my/entity"]),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities("my_segment_id3", &["my/entity"]),
        ],
    );
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
}

/// Collect the `rerun_segment_id` value of every returned row.
fn segment_ids(batches: Vec<RecordBatch>) -> Vec<String> {
    let mut ids = Vec::new();
    for batch in batches {
        if let Some(column) = batch.column_by_name(SEGMENT_ID_COL) {
            let column = column
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("rerun_segment_id should be a Utf8 column");
            ids.extend(column.iter().flatten().map(str::to_owned));
        }
    }
    ids
}

pub async fn scan_segment_table_filter(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    setup(&service, dataset_name).await;

    let scan_columns = async |columns: Vec<String>, sql_filter: &str| -> Vec<String> {
        let responses: Vec<_> = service
            .scan_segment_table(
                tonic::Request::new(ScanSegmentTableRequest {
                    columns,
                    sql_filter: sql_filter.to_owned(),
                })
                .with_entry_name(entry_name(dataset_name)),
            )
            .await
            .unwrap()
            .into_inner()
            .try_collect()
            .await
            .unwrap();
        segment_ids(
            responses
                .into_iter()
                .map(|resp| resp.data.unwrap().try_into().unwrap())
                .collect_vec(),
        )
    };
    let scan = async |sql_filter: &str| -> Vec<String> { scan_columns(vec![], sql_filter).await };

    // Unfiltered: one row per segment.
    let all = scan("")
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(all.len(), 3, "expected 3 segments, got: {all:?}");

    // Filtered to a single segment.
    let filtered = scan("rerun_segment_id = 'my_segment_id2'").await;
    assert_eq!(
        filtered,
        vec!["my_segment_id2".to_owned()],
        "got: {filtered:?}"
    );

    // IN list.
    let in_list = scan("rerun_segment_id IN ('my_segment_id1', 'my_segment_id3')")
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        in_list,
        ["my_segment_id1".to_owned(), "my_segment_id3".to_owned()]
            .into_iter()
            .collect(),
        "got: {in_list:?}"
    );

    // No match.
    assert!(scan("rerun_segment_id = 'nope'").await.is_empty());

    // Numeric comparison: the untyped SQL literal must be coerced to the column's type
    // (`rerun_num_chunks` is UInt64) instead of failing evaluation.
    let numeric = scan("rerun_num_chunks > 0")
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(numeric.len(), 3, "expected 3 segments, got: {numeric:?}");
    assert!(scan("rerun_num_chunks > 1000000").await.is_empty());

    // The filter may reference columns that are projected out: it applies before the projection.
    let projected = scan_columns(
        vec![SEGMENT_ID_COL.to_owned()],
        "rerun_num_chunks > 0 AND rerun_segment_id = 'my_segment_id2'",
    )
    .await;
    assert_eq!(
        projected,
        vec!["my_segment_id2".to_owned()],
        "got: {projected:?}"
    );
}

pub async fn scan_dataset_manifest_filter(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    setup(&service, dataset_name).await;

    let scan = async |sql_filter: &str| -> Vec<String> {
        let responses: Vec<_> = service
            .scan_dataset_manifest(
                tonic::Request::new(ScanDatasetManifestRequest {
                    columns: vec![],
                    sql_filter: sql_filter.to_owned(),
                })
                .with_entry_name(entry_name(dataset_name)),
            )
            .await
            .unwrap()
            .into_inner()
            .try_collect()
            .await
            .unwrap();
        segment_ids(
            responses
                .into_iter()
                .map(|resp| resp.data.unwrap().try_into().unwrap())
                .collect_vec(),
        )
    };

    // The manifest has one row per (segment, layer); here one layer each, so 3 rows.
    let all = scan("")
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(all.len(), 3, "expected 3 segments, got: {all:?}");

    let filtered = scan(&format!("{SEGMENT_ID_COL} = 'my_segment_id2'")).await;
    assert!(!filtered.is_empty(), "filter returned no rows");
    assert!(
        filtered.iter().all(|id| id == "my_segment_id2"),
        "filter leaked other segments, got: {filtered:?}"
    );

    assert!(scan(&format!("{SEGMENT_ID_COL} = 'nope'")).await.is_empty());

    // Columns whose public name differs from the server's internal (partition) name must be
    // rewritten server-side — a missed rewrite errors the whole scan.
    let renamed = scan("rerun_layer_name IS NOT NULL")
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(renamed.len(), 3, "expected 3 segments, got: {renamed:?}");

    // Numeric comparison: renamed column + untyped literal coerced to the column's type.
    // (`rerun_num_chunks` is nullable in the backing store, so only assert the no-match case —
    // it holds whether the value is set or null.)
    assert!(scan("rerun_num_chunks > 1000000").await.is_empty());
}
