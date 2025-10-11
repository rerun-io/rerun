use std::collections::{BTreeSet, HashMap, hash_map::Entry};
use std::path::Path;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_log_types::{EntryId, StoreKind};
use re_protos::{
    cloud::v1alpha1::{
        EntryKind, ScanPartitionTableResponse,
        ext::{DataSource, DatasetEntry, EntryDetails},
    },
    common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, PartitionId},
};

use crate::store::{Error, InMemoryStore, Layer, Partition};

pub struct Dataset {
    id: EntryId,
    name: String,
    partitions: HashMap<PartitionId, Partition>,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,
}

impl Dataset {
    pub fn new(id: EntryId, name: String) -> Self {
        Self {
            id,
            name,
            partitions: HashMap::default(),
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        }
    }

    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),
            kind: EntryKind::Dataset,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn as_dataset_entry(&self) -> DatasetEntry {
        DatasetEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Dataset,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            dataset_details: Default::default(),

            handle: DatasetHandle {
                id: Some(self.id),
                store_kind: StoreKind::Recording,
                url: url::Url::parse(&format!("memory:///{}", self.id)).expect("valid url"),
            },
        }
    }

    pub fn iter_store_handles(&self) -> impl Iterator<Item = &ChunkStoreHandle> {
        self.partitions
            .values()
            .flat_map(|partition| partition.iter_store_handles())
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let schemas = self.iter_store_handles().map(|store_handle| {
            let fields = store_handle.read().schema().arrow_fields();

            //TODO(ab): why is that needed again?
            Schema::new_with_metadata(fields, HashMap::default())
        });

        Schema::try_merge(schemas)
    }

    pub fn partition_ids(&self) -> impl Iterator<Item = PartitionId> {
        self.partitions.keys().cloned()
    }

    pub fn partition_table(&self) -> arrow::error::Result<RecordBatch> {
        let (partition_ids, last_updated_at, num_chunks, size_bytes): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = itertools::multiunzip(self.partitions.iter().map(|(partition_id, partition)| {
            (
                partition_id.to_string(),
                partition.last_updated_at().as_nanosecond() as i64,
                partition.num_chunks(),
                partition.size_bytes(),
            )
        }));

        let layers = vec![vec![DataSource::DEFAULT_LAYER.to_owned()]; partition_ids.len()];

        let storage_urls = partition_ids
            .iter()
            .map(|partition_id| vec![format!("memory:///{}/{partition_id}", self.id)])
            .collect();

        ScanPartitionTableResponse::create_dataframe(
            partition_ids,
            layers,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
    }

    pub fn layer_store_handle(
        &self,
        partition_id: &PartitionId,
        layer_name: &str,
    ) -> Option<&ChunkStoreHandle> {
        self.partitions
            .get(partition_id)
            .and_then(|partition| partition.layer(layer_name))
            .map(|layer| layer.store_handle())
    }

    pub fn add_layer(
        &mut self,
        partition_id: PartitionId,
        layer_name: String,
        store_handle: ChunkStoreHandle,
    ) {
        re_log::debug!(?partition_id, ?layer_name, "add_layer");

        self.partitions
            .entry(partition_id)
            .or_default()
            .insert_layer(layer_name, Layer::new(store_handle));

        self.updated_at = jiff::Timestamp::now();
    }

    /// Load a RRD using its recording id as partition id.
    pub fn load_rrd(
        &mut self,
        path: &Path,
        layer_name: Option<&str>,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<BTreeSet<PartitionId>, Error> {
        re_log::info!("Loading RRD: {}", path.display());
        let contents =
            ChunkStore::handle_from_rrd_filepath(&InMemoryStore::chunk_store_config(), path)
                .map_err(Error::RrdLoadingError)?;

        let layer_name = layer_name.unwrap_or(DataSource::DEFAULT_LAYER);

        let mut new_partition_ids = BTreeSet::default();

        for (store_id, chunk_store) in contents {
            if !store_id.is_recording() {
                continue;
            }

            let partition_id = PartitionId::new(store_id.recording_id().to_string());

            match self.partitions.entry(partition_id.clone()) {
                Entry::Vacant(entry) => {
                    new_partition_ids.insert(partition_id);

                    entry.insert(Partition::from_layer_data(layer_name, chunk_store));
                }
                Entry::Occupied(mut entry) => match on_duplicate {
                    IfDuplicateBehavior::Overwrite => {
                        re_log::info!("Overwriting {partition_id}");
                        entry.insert(Partition::from_layer_data(layer_name, chunk_store));
                    }
                    IfDuplicateBehavior::Skip => {
                        re_log::info!("Ignoring {partition_id}: it already exists");
                    }
                    IfDuplicateBehavior::Error => {
                        return Err(Error::DuplicateEntryNameError(partition_id.to_string()));
                    }
                },
            }
        }

        Ok(new_partition_ids)
    }
}
