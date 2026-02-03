use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches, prop,
};
use crate::utils::client::TestClient;
use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::SessionContext;
use futures::{StreamExt as _, TryStreamExt as _};
use re_chunk_store::IndexValue;
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::{EntityPath, TimeInt, TimeType};
use re_protos::cloud::v1alpha1::ext::DatasetEntry;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub async fn query_dataset_index_values_by_time_type<T: RerunCloudService>(
    service: Arc<T>,
    time_type: TimeType,
) {
    let tuid_prefix = match time_type {
        TimeType::TimestampNs => 1,
        TimeType::DurationNs => 10,
        TimeType::Sequence => 20,
    };

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        tuid_prefix,
        [
            LayerDefinition::simple_with_time(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
                1000,
                time_type,
            ),
            LayerDefinition::simple_with_time("my_segment_id2", &["my/entity"], 2000, time_type),
            LayerDefinition::properties(
                "my_segment_id1",
                [prop(
                    "text_log",
                    re_sdk_types::archetypes::TextLog::new("i'm segment 1"),
                )],
            )
            .layer_name("props"),
            LayerDefinition::simple_with_time(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
                3000,
                time_type,
            ),
        ],
    );

    let dataset_name = format!("dataset_{time_type}");
    let dataset_entry = service.create_dataset_entry_with_name(&dataset_name).await;
    service
        .register_with_dataset_name_blocking(&dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = TestClient { service };

    let tests = vec![
        (
            vec![
                ("my_segment_id1", vec![1020, 1040]),
                ("my_segment_id2", vec![2010, 2030]),
                ("my_segment_id3", vec![3010, 3020, 3030, 3040]),
            ],
            "all_valid_index_values",
            true,
        ),
        (
            vec![("my_segment_id1", vec![1020, 1040])],
            "single_segment",
            false,
        ),
        (
            vec![("my_segment_id4", vec![1020, 1040])],
            "unknown_segment",
            false,
        ),
    ];

    for (index_values, snapshot_name, check_schema) in tests {
        query_dataset_snapshot(
            client.clone(),
            &dataset_entry,
            index_values,
            &format!("query_index_values_{time_type}_{snapshot_name}"),
            time_type,
            check_schema,
        )
        .await;
    }
}

pub async fn query_dataset_index_values(service: impl RerunCloudService) {
    let service = Arc::new(service);
    query_dataset_index_values_by_time_type(service.clone(), TimeType::Sequence).await;
    query_dataset_index_values_by_time_type(service.clone(), TimeType::DurationNs).await;
    query_dataset_index_values_by_time_type(service.clone(), TimeType::TimestampNs).await;
}

// ---

async fn query_dataset_snapshot<T: RerunCloudService>(
    client: TestClient<T>,
    dataset_entry: &DatasetEntry,
    index_values: Vec<(&str, Vec<i64>)>,
    snapshot_name: &str,
    time_type: TimeType,
    check_schema: bool,
) {
    let index_values: BTreeMap<String, BTreeSet<IndexValue>> = index_values
        .into_iter()
        .map(|(idx, values)| {
            (
                idx.to_owned(),
                values.into_iter().map(TimeInt::new_temporal).collect(),
            )
        })
        .collect();

    let timeline_name = match time_type {
        TimeType::Sequence => "frame_nr",
        TimeType::DurationNs => "duration",
        TimeType::TimestampNs => "timestamp",
    };

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some(timeline_name.into()),
        ..Default::default()
    };

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        Some(Arc::new(index_values)),
        None,
    )
    .await
    .unwrap();

    let ctx = SessionContext::default();
    let plan = table_provider
        .scan(&ctx.state(), None, &[], None)
        .await
        .unwrap();
    let schema = plan.schema();

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

    for batch in &results {
        assert_eq!(batch.schema(), schema);
    }

    let results = if results.is_empty() {
        RecordBatch::new_empty(schema)
    } else {
        concat_record_batches(&results)
    };

    if check_schema {
        insta::assert_snapshot!(
            format!("{snapshot_name}_schema"),
            results.format_schema_snapshot()
        );
    }

    let filtered_results = results.horizontally_sorted().auto_sort_rows().unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}
