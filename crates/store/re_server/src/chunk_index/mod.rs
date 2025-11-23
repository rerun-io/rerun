mod index;
mod search;

use crate::rerun_cloud::SearchDatasetResponseStream;
use crate::store::Dataset;
use crate::store::Error as StoreError;
use ahash::{HashMap, HashMapExt as _};
use futures::StreamExt as _;
use re_chunk_store::ChunkStoreHandle;
use re_log_types::{EntityPath, EntryId};
use re_protos::cloud::v1alpha1::ext::{CreateIndexRequest, IndexConfig, SearchDatasetRequest};
use re_protos::cloud::v1alpha1::{CreateIndexResponse, SearchDatasetResponse};
use re_protos::common::v1alpha1::ext::PartitionId;
use re_types_core::ComponentIdentifier;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tonic::Status;
use tracing::instrument;
// Fields in an index table

pub const FIELD_RERUN_PARTITION_ID: &str = "rerun_partition_id"; // aka "segment"
pub const FIELD_RERUN_PARTITION_LAYER: &str = "rerun_partition_layer";
pub const FIELD_CHUNK_ID: &str = "chunk_id";
pub const FIELD_TIMEPOINT: &str = "timepoint";

// Indexed value
pub const FIELD_INSTANCE: &str = "instance";
// Position of the instance in the column cell
pub const FIELD_INSTANCE_ID: &str = "instance_id";

/// An index for a column of a dataset's chunks
pub struct Index {
    config: IndexConfig,
    // Mutex because we need to update the lance object after writing and checking out the latest version.
    lance_dataset: parking_lot::Mutex<lance::dataset::Dataset>,
}

/// All indexes for a dataset's chunks
///
/// Index creation behavior (mimics Rerun Cloud):
/// - Cannot create an index that already exists. Changing and index's parameters requires
///   deleting it first.
/// - Cannot create a index for a column that doesn't already have data, as we don't know
///   its type yet. This should be revisited to provide a better DX.
///
pub struct DatasetChunkIndexes {
    dataset_id: EntryId,
    // Created on demand with the first index, will be deleted when dropped
    dir: OnceLock<std::io::Result<tempfile::TempDir>>,
    // Nested hashmap to mimic the hierarchy in ChunkStoreHandle
    // we use an async lock as creating a new index involves I/O and async operations
    indexes: tokio::sync::RwLock<HashMap<EntityPath, HashMap<ComponentIdentifier, Arc<Index>>>>,
}

impl DatasetChunkIndexes {
    pub fn new(dataset_id: EntryId) -> Self {
        Self {
            dataset_id,
            dir: OnceLock::new(),
            indexes: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    // ---- GRPC API

    #[instrument(skip(self, dataset), fields(dataset_id = %self.dataset_id))]
    pub async fn create_index(
        &self,
        dataset: &Dataset,
        request: CreateIndexRequest,
    ) -> tonic::Result<tonic::Response<CreateIndexResponse>> {
        let config = request.config;

        // Lazily create the temp directory for this dataset's indexes if needed
        let temp_dir = self
            .dir
            .get_or_init(|| {
                tempfile::Builder::new()
                    .prefix(&format!("rerun-index-{}", self.dataset_id))
                    .tempdir()
            })
            .as_ref()
            .map_err(|e| StoreError::IndexingError(format!("Cannot create index directory {e}")))?;

        self.add_index(dataset, &config, temp_dir.path()).await?;

        Ok(tonic::Response::new(CreateIndexResponse {
            index: Some(config.into()),
            statistics_json: Default::default(),
            debug_info: None,
        }))
    }

    pub async fn search_dataset(
        dataset: &Dataset,
        request: SearchDatasetRequest,
    ) -> tonic::Result<tonic::Response<SearchDatasetResponseStream>> {
        let Some(index) = dataset
            .indexes()
            .get(
                &request.column.entity_path,
                &request.column.descriptor.component,
            )
            .await
        else {
            return Err(Status::invalid_argument("Column is not indexed"));
        };

        let stream = search::search_index(index, request).await?;

        let stream = stream.map(|batch| {
            batch
                .map(|batch| SearchDatasetResponse {
                    data: Some(batch.into()),
                })
                .map_err(Into::into)
        });

        Ok(tonic::Response::new(Box::pin(stream)))
    }

    // ----- Called by Dataset

    pub async fn chunks_loaded(
        &self,
        partition_id: PartitionId,
        store: ChunkStoreHandle,
        layer_name: &str,
        _overwritten: bool,
    ) -> Result<(), StoreError> {
        let mut worklist = vec![];

        {
            // Blocking lock: quickly get what we need
            let indexes = self.indexes.read().await;
            let store = store.read();

            for chunk in store.iter_chunks() {
                if let Some(entity_indexes) = indexes.get(chunk.entity_path()) {
                    // Find components by iterating on indexes (lower cardinality)
                    for (name, index) in entity_indexes {
                        if chunk.components().0.contains_key(name) {
                            // Needs indexing
                            worklist.push((
                                index.clone(),
                                partition_id.clone(),
                                layer_name.to_owned(),
                                chunk.clone(),
                            ));
                        }
                    }
                }
            }
        }

        for (index, partition_id, layer_name, chunk) in worklist {
            index
                .store_chunks(
                    vec![(partition_id.clone(), layer_name, chunk.clone())],
                    true,
                )
                .await?;
        }

        Ok(())
    }

    // ---- implementation

    /// Get the index for a path and component, if any.
    pub async fn get(
        &self,
        entity_path: &EntityPath,
        component: &ComponentIdentifier,
    ) -> Option<Arc<Index>> {
        let indexes = self.indexes.read().await;

        if let Some(path_indexes) = indexes.get(entity_path) {
            if let Some(component_index) = path_indexes.get(component) {
                return Some(component_index.clone());
            }
        }

        None
    }

    /// Add an index to a dataset
    pub(crate) async fn add_index(
        &self,
        dataset: &Dataset,
        config: &IndexConfig,
        dir: impl Into<&Path>,
    ) -> Result<Arc<Index>, StoreError> {
        let entity_path = &config.column.entity_path.clone();
        let component = &config.column.descriptor.component.clone();

        let path: PathBuf = dir
            .into()
            .join(entity_component_path(entity_path, component));

        let mut indexes = self.indexes.write().await;

        // Do we have it already?
        if let Some(path_indexes) = indexes.get(entity_path)
            && path_indexes.contains_key(component)
        {
            return Err(StoreError::IndexingError(
                "Index already exists, delete it first if you want to change its parameters"
                    .to_owned(),
            ));
        }

        let index = Arc::new(index::create_index(dataset, config, path).await?);

        // Register it and drop the lock
        indexes
            .entry(entity_path.clone())
            .or_default()
            .insert(*component, index.clone());
        drop(indexes);

        // Backfill existing data in the index
        let mut backfill = Vec::new();
        for (partition_id, partition) in dataset.partitions() {
            for (layer_name, layer) in partition.layers() {
                let store = layer.store_handle().read();
                for chunk in store.iter_chunks() {
                    if chunk.entity_path() == entity_path
                        && chunk.components().0.contains_key(component)
                    {
                        backfill.push((partition_id.clone(), layer_name.clone(), chunk.clone()));
                    }
                }
            }
        }

        index.store_chunks(backfill, true).await?;

        Ok(index)
    }
}

//---- Helper functions

/// Create a non-hierarchical path for an entity component
fn entity_component_path(entity_path: &EntityPath, component: &ComponentIdentifier) -> String {
    let mut result = String::new();

    for segment in entity_path.iter() {
        result.push_str(&segment.escaped_string()); // avoid non-alphanumeric characters in path
        result.push_str("--");
    }
    result.push_str(component.as_str());

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk_index::DatasetChunkIndexes;
    use arrow::array::{Array, record_batch};

    use re_log_types::{EntryId, StoreKind, TimelineName};
    use re_protos::cloud::v1alpha1::ext::{IndexColumn, IndexProperties, IndexQueryProperties};
    use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, ScanParameters};
    use re_types_core::ComponentDescriptor;

    const RRD: &str = "../../../tests/assets/rrd/snippets/views/timeseries.rrd";
    const ENTITY_PATH: &str = "/trig/sin";
    const TIMELINE: &str = "log_time";
    const COMPONENT: &str = "Scalars:scalars";

    #[tokio::test]
    async fn test_index_chunks() -> anyhow::Result<()> {
        let mut dataset = Dataset::new(
            EntryId::new(),
            "test-data".to_string(),
            StoreKind::Recording,
            Default::default(),
        );

        dataset
            .load_rrd(
                Path::new(RRD),
                None,
                IfDuplicateBehavior::Error,
                StoreKind::Recording,
            )
            .await?;

        //dump_dataset_info(&dataset);

        let config = IndexConfig {
            time_index: TimelineName::new(TIMELINE),
            column: IndexColumn {
                entity_path: EntityPath::from(ENTITY_PATH),
                descriptor: ComponentDescriptor {
                    component: ComponentIdentifier::new(COMPONENT),
                    archetype: None,
                    component_type: None,
                },
            },
            properties: IndexProperties::Btree,
        };

        let dir = tempfile::TempDir::new()?;
        let path = dir.path();

        let chunk_indexes = DatasetChunkIndexes::new(dataset.id());
        let index = chunk_indexes.add_index(&dataset, &config, path).await?;

        let lance = index.lance_dataset.lock().clone();

        // --- checks on the lance table contents
        assert_eq!(lance.count_rows(None).await?, 1256);

        let count = lance
            .scan()
            .filter("instance > 0")?
            .empty_project()?
            .with_row_id()
            .count_rows()
            .await?;

        // It's a sinusoid, half of the values are positive.
        assert_eq!(count, 1256 / 2);

        // --- test search function
        use arrow::datatypes as arrow_schema;

        let mut search = search::search_index(
            index,
            SearchDatasetRequest {
                column: IndexColumn {
                    entity_path: EntityPath::from(ENTITY_PATH),
                    descriptor: ComponentDescriptor {
                        component: ComponentIdentifier::new(COMPONENT),
                        archetype: None,
                        component_type: None,
                    },
                },
                query: record_batch!(("index", Float64, [0.0]))?,
                scan_parameters: ScanParameters {
                    columns: vec![
                        FIELD_TIMEPOINT.to_owned(),
                        FIELD_CHUNK_ID.to_owned(),
                        FIELD_INSTANCE.to_owned(),
                    ],
                    ..Default::default()
                },
                properties: IndexQueryProperties::Btree,
            },
        )
        .await?;

        while let Some(next) = search.next().await {
            let next = next?;
            assert_eq!(next.column(0).len(), 1);
            //println!("{:?}", next);
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn dump_dataset_info(dataset: &Dataset) {
        for (_, partition) in dataset.partitions() {
            for (lid, layer) in partition.layers() {
                for chunk in layer.store_handle().read().iter_chunks() {
                    println!(
                        "Chunk '{}' layer='{}' id = {}",
                        lid,
                        chunk.entity_path(),
                        chunk.id()
                    );
                    println!("  - timelines:");
                    for (tid, timeline) in chunk.timelines() {
                        println!("    - '{tid}' {:?}", timeline.timeline().datatype());
                    }
                    println!("  - components:");
                    for (_, component) in chunk.components().0.iter() {
                        println!(
                            "    - '{}' {} ({})",
                            component.descriptor.component,
                            component.list_array.value_type(),
                            component.list_array.len(),
                        );
                    }
                    println!();
                }
            }
        }
    }
}
