use std::collections::BTreeMap;

use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::Itertools as _;
use tonic::async_trait;
use url::Url;

use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::{
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DataSource, DataSourceKind, QueryTasksOnCompletionRequest,
        RegisterWithDatasetRequest, RegisterWithDatasetResponse,
        rerun_cloud_service_server::RerunCloudService,
    },
    common::v1alpha1::{IfDuplicateBehavior, TaskId},
    headers::RerunHeadersInjectorExt as _,
};
use re_types_core::AsComponents;

use crate::{
    RecordBatchExt as _, TempPath, TuidPrefix, create_nasty_recording,
    create_recording_with_properties, create_simple_recording,
};

/// Extension trait for the most common test setup tasks.
#[async_trait]
pub trait RerunCloudServiceExt: RerunCloudService {
    async fn create_dataset_entry_with_name(&self, dataset_name: &str);

    async fn register_with_dataset_name(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
    );
}

#[async_trait]
impl<T: RerunCloudService> RerunCloudServiceExt for T {
    async fn create_dataset_entry_with_name(&self, dataset_name: &str) {
        self.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        }))
        .await
        .expect("create_dataset_entry should succeed");
    }

    async fn register_with_dataset_name(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
    ) {
        let request = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate: IfDuplicateBehavior::Error as i32,
        })
        .with_entry_name(dataset_name)
        .expect("Failed to create a request");

        register_with_dataset(self, request).await;
    }
}

// ---

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

pub enum LayerType {
    /// See [`crate::utils::rerun::create_simple_recording`]
    Simple { entities: &'static [&'static str] },

    /// See [`crate::create_nasty_recording`]
    Nasty { entities: &'static [&'static str] },

    /// See [`crate::create_recording_with_properties`]
    #[expect(dead_code)] //TODO(ab): I'll need that in the next PR
    Properties {
        properties: BTreeMap<String, Vec<Box<dyn AsComponents>>>,
    },
}

impl LayerType {
    pub fn simple(entities: &'static [&'static str]) -> Self {
        Self::Simple { entities }
    }

    pub fn nasty(entities: &'static [&'static str]) -> Self {
        Self::Nasty { entities }
    }

    fn into_recording(
        self,
        tuid_prefix: TuidPrefix,
        partition_id: &str,
    ) -> anyhow::Result<TempPath> {
        match self {
            Self::Simple { entities } => {
                create_simple_recording(tuid_prefix, partition_id, entities)
            }

            Self::Nasty { entities } => create_nasty_recording(tuid_prefix, partition_id, entities),

            Self::Properties { properties } => create_recording_with_properties(
                tuid_prefix,
                partition_id,
                // TODO(ab): avoid this annoying conversion (this requires a change to
                // `create_recording_with_properties` which needs to be propagated to
                // `dataplatform`.
                properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.iter().map(|v| v.as_ref()).collect()))
                    .collect(),
            ),
        }
    }
}

pub struct LayerDefinition {
    pub partition_id: &'static str,
    pub layer_name: Option<&'static str>,
    pub layer_type: LayerType,
}

impl LayerDefinition {
    pub fn simple(partition_id: &'static str, entities: &'static [&'static str]) -> Self {
        Self {
            partition_id,
            layer_name: None,
            layer_type: LayerType::simple(entities),
        }
    }

    pub fn nasty(partition_id: &'static str, entities: &'static [&'static str]) -> Self {
        Self {
            partition_id,
            layer_name: None,
            layer_type: LayerType::nasty(entities),
        }
    }

    pub fn layer_name(mut self, layer_name: &'static str) -> Self {
        self.layer_name = Some(layer_name);
        self
    }
}

/// Utility to simplify the creation of data sources to register with a dataset.
///
/// This utility holds the [`TempPath`] instances, so it should not be dropped until the end of
/// the test, lest the recording files are prematurely cleaned up.
pub struct DataSourcesDefinition {
    layers: Vec<(Option<String>, TempPath)>,
}

impl DataSourcesDefinition {
    pub fn new(layers: impl IntoIterator<Item = LayerDefinition>) -> Self {
        Self {
            layers: layers
                .into_iter()
                .enumerate()
                .map(|(tuid_prefix, layer)| {
                    (
                        layer.layer_name.map(|s| s.to_owned()),
                        layer
                            .layer_type
                            .into_recording(tuid_prefix.saturating_add(1) as _, layer.partition_id)
                            .unwrap(),
                    )
                })
                .collect(),
        }
    }

    pub fn to_data_sources(&self) -> Vec<DataSource> {
        self.layers
            .iter()
            .map(|(layer_name, path)| DataSource {
                storage_url: Some(Url::from_file_path(path.as_path()).unwrap().to_string()),
                layer: layer_name.clone(),
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
