use crate::{RecordBatchExt as _, TempPath, create_nasty_recording, create_simple_recording};
use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::Itertools as _;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::DataSourceKind;
use re_protos::{
    cloud::v1alpha1::{
        DataSource, QueryTasksOnCompletionRequest, RegisterWithDatasetRequest,
        RegisterWithDatasetResponse,
    },
    common::v1alpha1::{IfDuplicateBehavior, TaskId},
    headers::RerunHeadersInjectorExt as _,
};
use url::Url;

#[expect(dead_code)]
pub async fn register_with_dataset_id(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    dataset_id: EntryId,
    data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
) {
    let request = tonic::Request::new(RegisterWithDatasetRequest {
        data_sources,
        on_duplicate: IfDuplicateBehavior::Error as i32,
    })
    .with_entry_id(dataset_id)
    .expect("Failed to create a request");

    register_with_dataset(service, request).await;
}

pub async fn register_with_dataset_name(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    dataset_name: &str,
    data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
) {
    let request = tonic::Request::new(RegisterWithDatasetRequest {
        data_sources,
        on_duplicate: IfDuplicateBehavior::Error as i32,
    })
    .with_entry_name(dataset_name)
    .expect("Failed to create a request");

    register_with_dataset(service, request).await;
}

async fn register_with_dataset(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    request: tonic::Request<RegisterWithDatasetRequest>,
) {
    let resp = service
        .register_with_dataset(request)
        .await
        .expect("register_with_dataset should succeed")
        .into_inner()
        .data
        .expect("data expected")
        .decode()
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

    let result = service
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
            let decoded = resp
                .data
                .expect("Expected response data")
                .decode()
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

// ---

pub struct LayerDefinition {
    pub partition_id: &'static str,
    pub layer_name: Option<&'static str>,
    pub entity_paths: &'static [&'static str],
}

/// Utility to simplify the creation of data sources to register with a dataset.
pub struct DataSourcesDefinition {
    layers: Vec<LayerDefinition>,
    paths: Option<Vec<TempPath>>,
}

impl DataSourcesDefinition {
    pub fn new(layers: impl IntoIterator<Item = LayerDefinition>) -> Self {
        Self {
            layers: layers.into_iter().collect(),
            paths: None,
        }
    }

    pub fn generate_simple(&mut self) {
        let paths = self
            .layers
            .iter()
            .enumerate()
            .map(|(tuid_prefix, l)| {
                create_simple_recording(
                    tuid_prefix.saturating_add(1) as _,
                    l.partition_id,
                    l.entity_paths,
                )
                .unwrap()
            })
            .collect_vec();
        self.paths = Some(paths);
    }

    pub fn generate_nasty(&mut self) {
        let paths = self
            .layers
            .iter()
            .enumerate()
            .map(|(tuid_prefix, l)| {
                create_nasty_recording(
                    tuid_prefix.saturating_add(1) as _,
                    l.partition_id,
                    l.entity_paths,
                )
                .unwrap()
            })
            .collect_vec();
        self.paths = Some(paths);
    }

    pub fn to_data_sources(&self) -> Vec<DataSource> {
        let Some(paths) = &self.paths else {
            panic!("generate_XXX() must be called before to_data_sources()");
        };

        self.layers
            .iter()
            .zip(paths.iter())
            .map(|(l, p)| DataSource {
                storage_url: Some(Url::from_file_path(p.as_path()).unwrap().to_string()),
                layer: l.layer_name.map(|l| l.to_owned()),
                typ: DataSourceKind::Rrd as i32,
            })
            .collect()
    }
}

// ---

/// Concatenate record batches.
///
/// This function implicitly tests the following properties:
/// - There is always at least one record batch, even if it is empty.
/// - All record batches have the same schema.
pub fn concat_record_batches(record_batches: &[RecordBatch]) -> RecordBatch {
    arrow::compute::concat_batches(
        record_batches
            .first()
            .expect("at least one record batch must be passed")
            .schema_ref(),
        record_batches,
    )
    .expect("record batches should be concatenable")
    .auto_sort_rows()
    .expect("record batches should be sortable")
}
