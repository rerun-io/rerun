use std::collections::BTreeMap;

use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::Itertools as _;
use re_log_types::{EntityPath, TimeType};
use re_protos::cloud::v1alpha1::ext::DatasetEntry;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    CreateDatasetEntryRequest, DataSource, QueryTasksOnCompletionRequest, QueryTasksResponse,
    RegisterWithDatasetRequest, RegisterWithDatasetResponse, ext,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::AsComponents;
use tonic::async_trait;
use url::Url;

use crate::utils::rerun::{
    create_recording_with_static_components, multi_chunked_entities_recording,
};
use crate::{
    RecordBatchTestExt as _, TempPath, TuidPrefix, create_nasty_recording,
    create_recording_with_embeddings, create_recording_with_properties,
    create_recording_with_scalars, create_recording_with_text, create_simple_recording,
};

/// Extension trait for the most common test setup tasks.
#[async_trait]
pub trait RerunCloudServiceExt: RerunCloudService {
    async fn create_dataset_entry_with_name(&self, dataset_name: &str) -> DatasetEntry;

    async fn register_with_dataset_name_blocking(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
    );

    async fn register_with_dataset_name_blocking_with_behavior(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
        on_duplicate: IfDuplicateBehavior,
    );

    async fn register_table_with_name(&self, table_name: &str, path: &std::path::Path);

    async fn unregister_from_dataset_name(
        &self,
        dataset_name: &str,
        segments_to_drop: &[&str],
        layers_to_drop: &[&str],
    ) -> tonic::Result<RecordBatch>;
}

#[async_trait]
impl<T: RerunCloudService> RerunCloudServiceExt for T {
    async fn create_dataset_entry_with_name(&self, dataset_name: &str) -> DatasetEntry {
        self.create_dataset_entry(tonic::Request::new(CreateDatasetEntryRequest {
            name: Some(dataset_name.to_owned()),
            id: None,
        }))
        .await
        .expect("create_dataset_entry should succeed")
        .into_inner()
        .dataset
        .expect("some dataset field expected")
        .try_into()
        .expect("conversion to ext::DatasetEntry should succeed")
    }

    async fn register_with_dataset_name_blocking(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
    ) {
        self.register_with_dataset_name_blocking_with_behavior(
            dataset_name,
            data_sources,
            IfDuplicateBehavior::Error,
        )
        .await;
    }

    async fn register_with_dataset_name_blocking_with_behavior(
        &self,
        dataset_name: &str,
        data_sources: Vec<re_protos::cloud::v1alpha1::DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) {
        let request = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate: re_protos::common::v1alpha1::IfDuplicateBehavior::from(on_duplicate)
                as i32,
        })
        .with_entry_name(dataset_name)
        .expect("Failed to create a request");

        register_with_dataset_blocking(self, request).await;
    }

    /// Helper to fire an [`UnregisterFromDatasetRequest`].
    ///
    /// `segments_to_drop` and `layers_to_drop` are combined using an *outer product*.
    /// Refer to [`UnregisterFromDatasetRequest`]'s to learn more about the semantics.
    ///
    /// [`UnregisterFromDatasetRequest`]: re_protos::cloud::v1alpha1::ext::UnregisterFromDatasetRequest
    async fn unregister_from_dataset_name(
        &self,
        dataset_name: &str,
        segments_to_drop: &[&str],
        layers_to_drop: &[&str],
    ) -> tonic::Result<RecordBatch> {
        let request = re_protos::cloud::v1alpha1::ext::UnregisterFromDatasetRequest {
            segments_to_drop: segments_to_drop
                .iter()
                .map(|id| (*id).to_owned().into())
                .collect(),
            layers_to_drop: layers_to_drop.iter().map(|s| (*s).to_owned()).collect(),
            force: false,
        };

        let request = tonic::Request::new(request.into())
            .with_entry_name(dataset_name)
            .expect("Failed to create a request");

        use futures::TryStreamExt as _;
        let responses: Vec<_> = self
            .unregister_from_dataset(request)
            .await?
            .into_inner()
            .try_collect()
            .await
            .expect("could not collect responses");

        let batches: Vec<RecordBatch> = responses
            .into_iter()
            .map(|resp| {
                resp.data
                    .expect("missing data in response")
                    .try_into()
                    .expect("could not convert response data to record batch")
            })
            .collect_vec();

        Ok(arrow::compute::concat_batches(
            batches
                .first()
                .expect("there should be at least one batch")
                .schema_ref(),
            &batches,
        )
        .expect("could not concatenate batches"))
    }

    async fn register_table_with_name(&self, table_name: &str, path: &std::path::Path) {
        let table_url =
            Url::from_directory_path(path).expect("Unable to create URL from directory path");
        let provider_details = re_protos::cloud::v1alpha1::ext::ProviderDetails::LanceTable(
            re_protos::cloud::v1alpha1::ext::LanceTable { table_url },
        );
        let request = re_protos::cloud::v1alpha1::ext::RegisterTableRequest {
            name: table_name.to_owned(),
            provider_details,
        };
        let request = tonic::Request::new(request.try_into().expect("Failed to convert request"));

        self.register_table(request)
            .await
            .expect("register table should succeed");
    }
}

// ---

/// Register data sources and wait for task completion, returning the task result batches.
pub async fn register_and_wait(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    request: tonic::Request<RegisterWithDatasetRequest>,
) -> Vec<RecordBatch> {
    let resp: RecordBatch = service
        .register_with_dataset(request)
        .await
        .expect("register_with_dataset should succeed")
        .into_inner()
        .data
        .expect("data expected")
        .try_into()
        .expect("record batch expected");

    // extract task ids from the response record batch
    let task_ids: Vec<TaskId> = resp
        .column_by_name(RegisterWithDatasetResponse::FIELD_TASK_ID)
        .expect("task_id column expected")
        .as_any()
        .downcast_ref::<arrow::array::StringArray>()
        .expect("task_id column should be a string array")
        .iter()
        .flatten()
        .map(|s| TaskId { id: s.to_owned() })
        .unique() // dups are possible because of batching partitions per task
        .collect();

    // Early return if no tasks were created (e.g., all partitions were skipped)
    if task_ids.is_empty() {
        return vec![];
    }

    service
        .query_tasks_on_completion(tonic::Request::new(QueryTasksOnCompletionRequest {
            ids: task_ids,
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
            resp.expect("Failed to get task completion response")
                .data
                .expect("Expected response data")
                .try_into()
                .expect("Failed to decode response data")
        })
        .collect()
}

async fn register_with_dataset_blocking(
    service: &impl re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
    request: tonic::Request<RegisterWithDatasetRequest>,
) {
    let task_results = register_and_wait(service, request).await;

    // Verify all tasks completed successfully
    for batch in &task_results {
        let status_col = batch
            .column_by_name(QueryTasksResponse::FIELD_EXEC_STATUS)
            .expect("exec_status column expected")
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .expect("exec_status should be string array");

        for i in 0..batch.num_rows() {
            let status = status_col.value(i);
            assert_eq!(
                status, "success",
                "Expected task to succeed, got status: {status}"
            );
        }
    }
}

// ---

pub enum LayerType {
    /// See [`crate::utils::rerun::create_simple_recording`]
    Simple {
        entities: &'static [&'static str],
        start_time: i64,
        time_type: TimeType,
    },

    /// See [`crate::create_nasty_recording`]
    Nasty { entities: &'static [&'static str] },

    /// See [`crate::create_recording_with_properties`]
    Properties {
        properties: BTreeMap<String, Vec<Box<dyn AsComponents>>>,
    },

    /// See [`crate::create_recording_with_static_components`]
    StaticComponents {
        components: BTreeMap<EntityPath, Box<dyn AsComponents>>,
    },

    /// See [`crate::create_recording_with_scalars`].
    Scalars { n: usize },

    /// See [`crate::create_recording_with_text`].
    Text,

    /// See [`crate::create_recording_with_embeddings`].
    Embeddings {
        embeddings: u32,
        embeddings_per_row: u32,
    },

    /// See [`crate::create_simple_blueprint`]
    SimpleBlueprint,

    /// See [`crate::utils::rerun::multi_chunked_entities_recording`]
    MultiChunkedEntities { entities: &'static [&'static str] },
}

impl LayerType {
    pub fn simple(entities: &'static [&'static str]) -> Self {
        Self::Simple {
            entities,
            start_time: 0,
            time_type: TimeType::Sequence,
        }
    }

    pub fn simple_with_time(
        entities: &'static [&'static str],
        start_time: i64,
        time_type: TimeType,
    ) -> Self {
        Self::Simple {
            entities,
            start_time,
            time_type,
        }
    }

    pub fn nasty(entities: &'static [&'static str]) -> Self {
        Self::Nasty { entities }
    }

    pub fn properties(
        properties: impl IntoIterator<Item = (String, Box<dyn AsComponents>)>,
    ) -> Self {
        Self::Properties {
            properties: properties.into_iter().map(|(k, v)| (k, vec![v])).collect(),
        }
    }

    pub fn static_components(
        components: impl IntoIterator<Item = (EntityPath, Box<dyn AsComponents>)>,
    ) -> Self {
        Self::StaticComponents {
            components: components.into_iter().collect(),
        }
    }

    pub fn scalars(n: usize) -> Self {
        Self::Scalars { n }
    }

    pub fn text() -> Self {
        Self::Text
    }

    pub fn embeddings(embeddings: u32, embeddings_per_row: u32) -> Self {
        Self::Embeddings {
            embeddings,
            embeddings_per_row,
        }
    }

    pub fn simple_blueprint() -> Self {
        Self::SimpleBlueprint
    }

    fn into_recording(self, tuid_prefix: TuidPrefix, segment_id: &str) -> anyhow::Result<TempPath> {
        match self {
            Self::Simple {
                entities,
                start_time,
                time_type,
            } => create_simple_recording(tuid_prefix, segment_id, entities, start_time, time_type),

            Self::Nasty { entities } => create_nasty_recording(tuid_prefix, segment_id, entities),

            Self::Properties { properties } => create_recording_with_properties(
                tuid_prefix,
                segment_id,
                // TODO(ab): avoid this annoying conversion
                properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.iter().map(|v| v.as_ref()).collect()))
                    .collect(),
            ),

            Self::StaticComponents { components } => {
                create_recording_with_static_components(tuid_prefix, segment_id, components)
            }

            Self::Scalars { n } => create_recording_with_scalars(tuid_prefix, segment_id, n),

            Self::Text => create_recording_with_text(tuid_prefix, segment_id),

            Self::Embeddings {
                embeddings,
                embeddings_per_row,
            } => create_recording_with_embeddings(
                tuid_prefix,
                segment_id,
                embeddings,
                embeddings_per_row,
            ),

            Self::SimpleBlueprint => crate::create_simple_blueprint(tuid_prefix, segment_id),

            Self::MultiChunkedEntities { entities } => {
                multi_chunked_entities_recording(tuid_prefix, segment_id, entities)
            }
        }
    }

    pub fn multi_chunked_entities(entities: &'static [&'static str]) -> Self {
        Self::MultiChunkedEntities { entities }
    }
}

pub struct LayerDefinition {
    pub segment_id: &'static str,
    pub layer_name: Option<&'static str>,
    pub layer_type: LayerType,
}

impl LayerDefinition {
    /// A simple layer with the provided entities
    pub fn simple(segment_id: &'static str, entities: &'static [&'static str]) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::simple(entities),
        }
    }

    pub fn simple_with_time(
        segment_id: &'static str,
        entities: &'static [&'static str],
        start_time: i64,
        time_type: TimeType,
    ) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::simple_with_time(entities, start_time, time_type),
        }
    }

    /// A layer with a nasty chunk representation for the provided entities.
    pub fn nasty(segment_id: &'static str, entities: &'static [&'static str]) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::nasty(entities),
        }
    }

    /// A layer with just the provided properties.
    pub fn properties(
        segment_id: &'static str,
        properties: impl IntoIterator<Item = (String, Box<dyn AsComponents>)>,
    ) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::properties(properties),
        }
    }

    pub fn static_components(
        segment_id: &'static str,
        components: impl IntoIterator<Item = (EntityPath, Box<dyn AsComponents>)>,
    ) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::static_components(components),
        }
    }

    /// A simple layer with a bunch of scalars, for testing B-Tree indexes.
    pub fn scalars(segment_id: &'static str) -> Self {
        Self {
            segment_id,
            layer_name: None,
            // TODO(cmc): we can always expose `n` later, if and when it's useful.
            layer_type: LayerType::scalars(10),
        }
    }

    /// A simple layer with a bunch of text, for testing FTS indexes.
    pub fn text(segment_id: &'static str) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::text(),
        }
    }

    /// A simple layer with a bunch of embeddings, for testing Vector indexes.
    pub fn embeddings(segment_id: &'static str, embeddings: u32, embeddings_per_row: u32) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::embeddings(embeddings, embeddings_per_row),
        }
    }

    pub fn simple_blueprint(segment_id: &'static str) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::simple_blueprint(),
        }
    }

    pub fn layer_name(mut self, layer_name: &'static str) -> Self {
        self.layer_name = Some(layer_name);
        self
    }

    pub fn multi_chunked_entities(
        segment_id: &'static str,
        entities: &'static [&'static str],
    ) -> Self {
        Self {
            segment_id,
            layer_name: None,
            layer_type: LayerType::multi_chunked_entities(entities),
        }
    }
}

/// Helper function to construct property tuples
pub fn prop(
    key: impl Into<String>,
    value: impl AsComponents + 'static,
) -> (String, Box<dyn AsComponents>) {
    (key.into(), Box::new(value) as Box<dyn AsComponents>)
}

/// Utility to simplify the creation of data sources to register with a dataset.
///
/// This utility holds the [`TempPath`] instances, so it should not be dropped until the end of
/// the test, lest the recording files are prematurely cleaned up.
pub struct DataSourcesDefinition {
    layers: Vec<(Option<String>, TempPath)>,
}

impl DataSourcesDefinition {
    /// Create layers with the provided definitions.
    ///
    /// The provided `tuid_prefix` is used for the first layer and then incremented.
    ///
    /// Note: we require an explicit prefix, otherwise using two `DataSourcesDefinition`s in the
    /// same test will cause a chunk id conflict, which is UB :true-story:
    pub fn new_with_tuid_prefix(
        tuid_prefix: TuidPrefix,
        layers: impl IntoIterator<Item = LayerDefinition>,
    ) -> Self {
        Self {
            layers: layers
                .into_iter()
                .enumerate()
                .map(|(tuid_prefix_increment, layer)| {
                    (
                        layer.layer_name.map(|s| s.to_owned()),
                        layer
                            .layer_type
                            .into_recording(
                                tuid_prefix.saturating_add(tuid_prefix_increment as _),
                                layer.segment_id,
                            )
                            .unwrap(),
                    )
                })
                .collect(),
        }
    }

    pub fn to_data_sources_ext(&self) -> Vec<ext::DataSource> {
        self.layers
            .iter()
            .map(|(layer_name, path)| ext::DataSource {
                storage_url: Url::from_file_path(path.as_path()).unwrap(),
                layer: layer_name
                    .clone()
                    .unwrap_or_else(|| ext::DataSource::DEFAULT_LAYER.to_owned()),
                is_prefix: false,
                kind: ext::DataSourceKind::Rrd,
            })
            .collect()
    }

    pub fn to_data_sources(&self) -> Vec<DataSource> {
        self.to_data_sources_ext()
            .into_iter()
            .map(Into::into)
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
