use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};
use crate::utils::client::create_test_client;
use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::execution::SessionState;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::{Expr, SessionConfig, SessionContext, col, lit};
use futures::{StreamExt as _, TryStreamExt as _};
use re_datafusion::{DataframeClientAPI, DataframeQueryTableProvider};
use re_log_types::EntityPath;
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
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
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
        None, // arrow_schema — let the provider fetch it
        None, // trace_headers
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

pub async fn query_dataset_with_limit(service: impl RerunCloudService) {
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
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
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
        None, // arrow_schema — let the provider fetch it
        None, // trace_headers
    )
    .await
    .unwrap();

    // We need to set 1 target partition, otherwise we will not get exactly the limit we expect
    let config = SessionConfig::default().with_target_partitions(1);
    let ctx = SessionContext::new_with_config(config);
    let state = ctx.state();

    // First, get the total row count without any limit
    let total_rows = execute_with_limit(&table_provider, &ctx, &state, None).await;
    assert!(
        total_rows > 1,
        "Test requires more than 1 row to be meaningful, got {total_rows}"
    );

    // Limit of 1 should return exactly 1 row
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(1)).await;
    assert_eq!(rows, 1, "limit=1 should return exactly 1 row");

    // Limit smaller than total should return exactly that many rows
    let small_limit = total_rows / 2;
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(small_limit)).await;
    assert_eq!(
        rows, small_limit,
        "limit={small_limit} should return exactly {small_limit} rows"
    );

    // Limit equal to total should return all rows
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(total_rows)).await;
    assert_eq!(
        rows, total_rows,
        "limit={total_rows} should return all {total_rows} rows"
    );

    // Limit larger than total should return all rows
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(total_rows + 100)).await;
    assert_eq!(
        rows, total_rows,
        "limit larger than total should return all {total_rows} rows"
    );

    // SAFETY:
    // This is simply a test
    unsafe {
        match original_env {
            Some(val) => std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", val),
            None => std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED"),
        }
    }
}

async fn execute_with_limit<T: DataframeClientAPI>(
    table_provider: &DataframeQueryTableProvider<T>,
    ctx: &SessionContext,
    state: &SessionState,
    limit: Option<usize>,
) -> usize {
    let plan = table_provider
        .scan(state, None, &[lit(true)], limit)
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

    results.iter().map(|batch| batch.num_rows()).sum()
}

// ---

async fn query_dataset_snapshot<T: DataframeClientAPI>(
    table_provider: &DataframeQueryTableProvider<T>,
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

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        results.format_schema_snapshot()
    );

    // these columns are not stable, so we cannot snapshot them
    let filtered_results = results.horizontally_sorted().auto_sort_rows().unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}
