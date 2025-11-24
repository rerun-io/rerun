use arrow::array::RecordBatch;
use futures::TryStreamExt as _;
use itertools::Itertools as _;
use re_protos::{
    cloud::v1alpha1::{
        ScanDatasetManifestRequest, ScanPartitionTableRequest, ScanPartitionTableResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    headers::RerunHeadersInjectorExt as _,
};

use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, prop,
};

pub async fn test_partition_table_column_projections(service: impl RerunCloudService) {
    test_column_projections(service, &projected_partition_table_batch, "partition_table").await;
}

pub async fn test_dataset_manifest_column_projections(service: impl RerunCloudService) {
    test_column_projections(
        service,
        &projected_dataset_manifest_batch,
        "dataset_manifest",
    )
    .await;
}

async fn test_column_projections<T>(
    service: T,
    project_fn: &impl AsyncFn(&T, Vec<String>, &str) -> Vec<String>,
    case_name: &'static str,
) where
    T: RerunCloudService,
{
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::properties(
            "my_partition_id",
            [
                prop(
                    "text_log",
                    re_types::archetypes::TextLog::new("i'm partition 1"),
                ),
                prop(
                    "points",
                    re_types::archetypes::Points2D::new([(1., 2.), (3., 4.)]),
                ),
            ],
        )],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

    //
    // check we get all columns when no projection is specified
    //

    let all_columns = project_fn(&service, vec![], dataset_name).await;
    insta::assert_debug_snapshot!(format!("{case_name}_all_columns"), &all_columns);

    //
    // we can project a base column
    //

    let partition_id_columns = project_fn(
        &service,
        vec![ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned()],
        dataset_name,
    )
    .await;

    assert_eq!(
        partition_id_columns,
        vec![ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned()],
        "the projection should have been applied"
    );

    //
    // we can project a property column
    //

    let prop_col = "property:points:Points2D:positions".to_owned();
    let partition_id_columns = project_fn(&service, vec![prop_col.clone()], dataset_name).await;

    assert_eq!(
        partition_id_columns,
        vec![prop_col],
        "the projection should have been applied"
    );

    //
    // check for order preservation
    //

    let prop_col = "property:points:Points2D:positions".to_owned();
    let ordered_columns = project_fn(
        &service,
        vec![
            prop_col.clone(),
            ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned(),
        ],
        dataset_name,
    )
    .await;

    assert_eq!(
        ordered_columns,
        vec![
            prop_col,
            ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned(),
        ],
        "the column order should be preserved"
    );

    //
    // check for unknown column
    //

    let result = service
        .scan_partition_table(
            tonic::Request::new(ScanPartitionTableRequest {
                columns: vec!["unknown_column".to_owned()],
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await;

    match result {
        Err(status) => {
            assert_eq!(status.code(), tonic::Code::InvalidArgument);
            assert!(status.message().contains("unknown_column"));
            assert!(status.message().contains("not found"));
        }
        Ok(_) => panic!("expected InvalidArgument error for unknown column"),
    }

    //
    // check for duplicate column
    //

    let result = service
        .scan_partition_table(
            tonic::Request::new(ScanPartitionTableRequest {
                columns: vec![
                    ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned(),
                    ScanPartitionTableResponse::FIELD_PARTITION_ID.to_owned(),
                ],
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await;

    match result {
        Err(status) => {
            assert_eq!(status.code(), tonic::Code::InvalidArgument);
            assert!(
                status
                    .message()
                    .contains(ScanPartitionTableResponse::FIELD_PARTITION_ID)
            );
            assert!(status.message().contains("twice") || status.message().contains("duplicate"));
        }
        Ok(_) => panic!("expected InvalidArgument error for duplicate column"),
    }
}

async fn projected_partition_table_batch(
    service: &impl RerunCloudService,
    column_projection: Vec<String>,
    dataset_name: &str,
) -> Vec<String> {
    let responses: Vec<_> = service
        .scan_partition_table(
            tonic::Request::new(ScanPartitionTableRequest {
                columns: column_projection,
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches: Vec<RecordBatch> = responses
        .into_iter()
        .map(|resp| resp.data.unwrap().try_into().unwrap())
        .collect_vec();

    let batch = arrow::compute::concat_batches(
        batches
            .first()
            .expect("there should be at least one batch")
            .schema_ref(),
        &batches,
    )
    .unwrap();

    batch
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().to_owned())
        .collect_vec()
}

async fn projected_dataset_manifest_batch(
    service: &impl RerunCloudService,
    column_projection: Vec<String>,
    dataset_name: &str,
) -> Vec<String> {
    let responses: Vec<_> = service
        .scan_dataset_manifest(
            tonic::Request::new(ScanDatasetManifestRequest {
                columns: column_projection,
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_collect()
        .await
        .unwrap();

    let batches: Vec<RecordBatch> = responses
        .into_iter()
        .map(|resp| resp.data.unwrap().try_into().unwrap())
        .collect_vec();

    let batch = arrow::compute::concat_batches(
        batches
            .first()
            .expect("there should be at least one batch")
            .schema_ref(),
        &batches,
    )
    .unwrap();

    batch
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().to_owned())
        .collect_vec()
}
