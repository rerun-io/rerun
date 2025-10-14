use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::Itertools as _;

use re_log_types::EntryId;
use re_protos::{
    cloud::v1alpha1::{
        QueryTasksOnCompletionRequest, RegisterWithDatasetRequest, RegisterWithDatasetResponse,
    },
    common::v1alpha1::{IfDuplicateBehavior, TaskId},
    headers::RerunHeadersInjectorExt as _,
};

// Helper function that calls the asynchronous register_with_dataset and blocks until the
// corresponding tasks have completed.
pub async fn register_with_dataset(
    fe: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    dataset_id: EntryId,
    data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
) {
    let resp: RecordBatch = fe
        .register_with_dataset(
            tonic::Request::new(RegisterWithDatasetRequest {
                data_sources,
                on_duplicate: IfDuplicateBehavior::Error as i32,
            })
            .with_entry_id(dataset_id)
            .expect("Failed to get catalog handler"),
        )
        .await
        .expect("register_with_dataset should succeed")
        .into_inner()
        .data
        .expect("data expected")
        .try_into()
        .expect("record batch expected");

    // extract task ids from the response record batch
    let task_ids = {
        resp.column_by_name(RegisterWithDatasetResponse::TASK_ID)
            .expect("task_id column expected")
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .expect("task_id column should be a string array")
            .iter()
            .flatten()
            .map(|s| TaskId { id: s.to_owned() })
            .unique() // dups are possible because of batching partitions per task
            .collect::<Vec<_>>()
    };

    let result = fe
        .query_tasks_on_completion(tonic::Request::new(QueryTasksOnCompletionRequest {
            ids: task_ids.clone(),
            timeout: Some(prost_types::Duration {
                seconds: 20,
                nanos: 0,
            }),
        }))
        .await
        .expect("should get query results")
        .into_inner()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|resp| {
            let resp = resp.expect("Failed to get task completion response");
            let decoded: RecordBatch = resp
                .data
                .expect("Expected response data")
                .try_into()
                .expect("Failed to decode response data");
            let task_id = decoded
                .column_by_name("task_id")
                .expect("task_id column expected")
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .expect("task_id column should be a string array")
                .value(0); // Get first value
            TaskId {
                id: task_id.to_owned(),
            }
        })
        .collect_vec();

    let returned_task_ids: std::collections::HashSet<_> = result.iter().collect();
    for tid in &task_ids {
        assert!(
            returned_task_ids.contains(tid),
            "Expected task {} to be in the results",
            tid.id
        );
    }
}
