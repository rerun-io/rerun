use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};
use crate::utils::client::create_test_client;
use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::execution::SessionState;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::{Expr, SessionContext, col, lit};
use futures::{StreamExt as _, TryStreamExt as _};
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::EntityPath;
use re_protos::cloud::v1alpha1::QueryDatasetResponse;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;

pub async fn query_dataset_simple_filter(service: impl RerunCloudService) {
    #![expect(unsafe_code)]
    let original_env = std::env::var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED").ok();

    // SAFETY:
    // This is simply a test
    unsafe { std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "3") };

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
            ),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name(dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = create_test_client(service).await;
    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        ..Default::default()
    };

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        None,
    )
    .await
    .unwrap();

    let ctx = SessionContext::default();
    let state = ctx.state();

    let tests = vec![
        (lit(true), "default"),
        (
            col("rerun_segment_id").eq(lit("my_segment_id2")),
            "seg_id_eq",
        ),
        (col("frame_nr").eq(lit(50)), "frame_nr_eq"),
    ];

    for (filter, snapshot_name) in tests {
        query_dataset_snapshot(
            &table_provider,
            &ctx,
            &state,
            filter,
            &format!("simple_dataset_{snapshot_name}"),
        )
        .await;
    }

    // SAFETY:
    // This is simply a test
    unsafe {
        match original_env {
            Some(val) => std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", val),
            None => std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED"),
        }
    }
}

// ---

async fn query_dataset_snapshot(
    table_provider: &DataframeQueryTableProvider,
    ctx: &SessionContext,
    state: &SessionState,
    filter: Expr,
    snapshot_name: &str,
) {
    let plan = table_provider
        .scan(state, None, &[filter], None)
        .await
        .unwrap();

    let num_partitions = plan.output_partitioning().partition_count();
    let results = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let stream = futures::stream::iter(results);

    let results: Vec<RecordBatch> = stream
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();

    let results = concat_record_batches(&results);

    // TODO(tsaucer) uncomment after all other parts are working
    // insta::assert_snapshot!(
    //     format!("{snapshot_name}_schema"),
    //     results.format_schema_snapshot()
    // );

    // these columns are not stable, so we cannot snapshot them
    let filtered_results = results
        .remove_columns(&[QueryDatasetResponse::FIELD_CHUNK_KEY])
        .auto_sort_rows()
        .unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}
