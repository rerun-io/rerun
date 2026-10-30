//! End-to-end coverage for segment-ID scan hints on `ScanSegmentTable` and
//! `ScanDatasetManifest`, and layer-name scan hints on `ScanDatasetManifest`.
//! Runs against both the OSS server (`re_server`) and the cloud frontend.

use std::collections::BTreeSet;

use arrow::array::{RecordBatch, StringArray};
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_arrow_util::RecordBatchExt as _;
use re_protos::cloud::v1alpha1::ext::{ScanDatasetManifestDataframe, ScanSegmentTableDataframe};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    LayerNameFilter, LayerNameList, ScanDatasetManifestRequest, ScanSegmentTableRequest,
    SegmentIdFilter, SegmentIdList, layer_name_filter, segment_id_filter,
};
use re_protos::headers::RerunHeadersInjectorExt as _;

use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, entry_name,
};

const SEGMENT_ID_COL: &str = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;
const SCHEMA_SHA256_COL: &str = ScanDatasetManifestDataframe::COLUMN_RERUN_SCHEMA_SHA256_NAME;

fn scan_only(ids: &[&str]) -> SegmentIdFilter {
    SegmentIdFilter {
        strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
            segment_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        })),
    }
}

fn skip(ids: &[&str]) -> SegmentIdFilter {
    SegmentIdFilter {
        strategy: Some(segment_id_filter::Strategy::Skip(SegmentIdList {
            segment_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        })),
    }
}

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

fn segment_ids(batches: Vec<RecordBatch>) -> Vec<String> {
    let mut ids = Vec::new();
    for batch in batches {
        if batch.column_by_name(SEGMENT_ID_COL).is_some() {
            let column = batch
                .try_get_column_as::<StringArray>(SEGMENT_ID_COL)
                .unwrap();
            ids.extend(column.iter().flatten().map(str::to_owned));
        }
    }
    ids
}

pub async fn scan_segment_table_filter(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    setup(&service, dataset_name).await;

    let scan_columns =
        async |columns: Vec<String>, segment_id_filter: Option<SegmentIdFilter>| -> Vec<String> {
            let responses: Vec<_> = service
                .scan_segment_table(
                    tonic::Request::new(ScanSegmentTableRequest {
                        columns,
                        segment_id_filter,
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
    let scan = async |filter| -> Vec<String> { scan_columns(vec![], filter).await };

    let all = scan(None)
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(all.len(), 3, "expected 3 segments, got: {all:?}");

    let filtered = scan(Some(scan_only(&["my_segment_id2"]))).await;
    assert_eq!(filtered, vec!["my_segment_id2".to_owned()]);

    let in_list = scan(Some(scan_only(&["my_segment_id1", "my_segment_id3"])))
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        in_list,
        ["my_segment_id1".to_owned(), "my_segment_id3".to_owned()]
            .into_iter()
            .collect()
    );

    assert!(scan(Some(scan_only(&["nope"]))).await.is_empty());
    assert!(scan(Some(scan_only(&[]))).await.is_empty());
    assert_eq!(scan(Some(skip(&[]))).await.len(), 3);
    assert_eq!(
        scan(Some(SegmentIdFilter { strategy: None })).await.len(),
        3
    );

    let skipped = scan(Some(skip(&["my_segment_id2"])))
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        skipped,
        ["my_segment_id1".to_owned(), "my_segment_id3".to_owned()]
            .into_iter()
            .collect()
    );

    let projected = scan_columns(
        vec![SEGMENT_ID_COL.to_owned()],
        Some(scan_only(&["my_segment_id2"])),
    )
    .await;
    assert_eq!(projected, vec!["my_segment_id2".to_owned()]);
}

pub async fn scan_dataset_manifest_filter(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    setup(&service, dataset_name).await;

    let scan = async |segment_id_filter: Option<SegmentIdFilter>| -> Vec<String> {
        let responses: Vec<_> = service
            .scan_dataset_manifest(
                tonic::Request::new(ScanDatasetManifestRequest {
                    columns: vec![],
                    segment_id_filter,
                    layer_name_filter: None,
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

    let all = scan(None)
        .await
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(all.len(), 3, "expected 3 segments, got: {all:?}");

    let filtered = scan(Some(scan_only(&["my_segment_id2"]))).await;
    assert!(!filtered.is_empty(), "filter returned no rows");
    assert!(filtered.iter().all(|id| id == "my_segment_id2"));

    assert!(scan(Some(scan_only(&["nope"]))).await.is_empty());
    assert!(scan(Some(scan_only(&[]))).await.is_empty());
    assert_eq!(scan(Some(skip(&[]))).await.len(), 3);
    assert_eq!(
        scan(Some(SegmentIdFilter { strategy: None })).await.len(),
        3
    );

    let skipped = scan(Some(skip(&["my_segment_id1", "my_segment_id3"]))).await;
    assert_eq!(skipped, vec!["my_segment_id2".to_owned()]);
}

fn scan_only_layers(names: &[&str]) -> LayerNameFilter {
    LayerNameFilter {
        strategy: Some(layer_name_filter::Strategy::ScanOnly(LayerNameList {
            layer_names: names.iter().map(|name| (*name).to_owned()).collect(),
        })),
    }
}

fn skip_layers(names: &[&str]) -> LayerNameFilter {
    LayerNameFilter {
        strategy: Some(layer_name_filter::Strategy::Skip(LayerNameList {
            layer_names: names.iter().map(|name| (*name).to_owned()).collect(),
        })),
    }
}

/// `(segment, layer)` pairs, as `"segment/layer"` for readable assertions.
fn segment_layers(batches: Vec<RecordBatch>) -> BTreeSet<String> {
    batches
        .into_iter()
        .flat_map(|batch| {
            let segment_ids = ScanDatasetManifestDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch)
                .unwrap();
            let layer_names = ScanDatasetManifestDataframe::COLUMN_RERUN_LAYER_NAME
                .extract(&batch)
                .unwrap();
            std::iter::zip(segment_ids.iter(), layer_names.iter())
                .map(|(segment, layer)| format!("{segment}/{layer}"))
                .collect_vec()
        })
        .collect()
}

fn pairs(pairs: &[&str]) -> BTreeSet<String> {
    pairs.iter().map(|pair| (*pair).to_owned()).collect()
}

pub async fn scan_dataset_manifest_layer_filter(service: impl RerunCloudService) {
    let dataset_name = "my_dataset";
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("seg1", &["my/entity"]),
            LayerDefinition::simple("seg1", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("seg2", &["my/entity"]),
            LayerDefinition::simple("seg2", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("seg3", &["my/entity"]),
        ],
    );
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let scan_columns = async |columns: Vec<String>,
                              segment_id_filter: Option<SegmentIdFilter>,
                              layer_name_filter: Option<LayerNameFilter>|
           -> Vec<RecordBatch> {
        let responses: Vec<_> = service
            .scan_dataset_manifest(
                tonic::Request::new(ScanDatasetManifestRequest {
                    columns,
                    segment_id_filter,
                    layer_name_filter,
                })
                .with_entry_name(entry_name(dataset_name)),
            )
            .await
            .unwrap()
            .into_inner()
            .try_collect()
            .await
            .unwrap();
        responses
            .into_iter()
            .map(|resp| resp.data.unwrap().try_into().unwrap())
            .collect_vec()
    };
    let scan = async |segment_id_filter, layer_name_filter| -> BTreeSet<String> {
        segment_layers(scan_columns(vec![], segment_id_filter, layer_name_filter).await)
    };

    let all = pairs(&[
        "seg1/base",
        "seg1/extra",
        "seg2/base",
        "seg2/extra",
        "seg3/base",
    ]);
    assert_eq!(scan(None, None).await, all);

    assert_eq!(
        scan(None, Some(scan_only_layers(&["extra"]))).await,
        pairs(&["seg1/extra", "seg2/extra"])
    );
    assert_eq!(
        scan(None, Some(skip_layers(&["extra"]))).await,
        pairs(&["seg1/base", "seg2/base", "seg3/base"])
    );
    assert_eq!(
        scan(None, Some(scan_only_layers(&["base", "extra"]))).await,
        all
    );

    assert!(
        scan(None, Some(scan_only_layers(&["nope"])))
            .await
            .is_empty()
    );
    assert!(scan(None, Some(scan_only_layers(&[]))).await.is_empty());
    assert_eq!(scan(None, Some(skip_layers(&[]))).await, all);
    assert_eq!(
        scan(None, Some(LayerNameFilter { strategy: None })).await,
        all
    );

    // Both filters must hold: a cross product of segments and layers.
    assert_eq!(
        scan(
            Some(scan_only(&["seg2", "seg3"])),
            Some(scan_only_layers(&["extra"]))
        )
        .await,
        pairs(&["seg2/extra"])
    );
    assert_eq!(
        scan(Some(skip(&["seg1"])), Some(skip_layers(&["extra"]))).await,
        pairs(&["seg2/base", "seg3/base"])
    );

    // Filtering on a column that isn't projected.
    let projected = scan_columns(
        vec![SCHEMA_SHA256_COL.to_owned()],
        None,
        Some(scan_only_layers(&["extra"])),
    )
    .await;
    assert_eq!(
        projected.iter().map(RecordBatch::num_rows).sum::<usize>(),
        2
    );
    for batch in &projected {
        assert_eq!(
            batch
                .schema()
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect_vec(),
            [SCHEMA_SHA256_COL]
        );
    }
}
