use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use arrow::error::ArrowError;

use itertools::Either;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_log_types::{EntryId, StoreKind};
use re_protos::{
    cloud::v1alpha1::{
        EntryKind, ScanDatasetManifestResponse, ScanPartitionTableResponse,
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

    pub fn partition(&self, partition_id: &PartitionId) -> Result<&Partition, Error> {
        self.partitions
            .get(partition_id)
            .ok_or_else(|| Error::PartitionIdNotFound(partition_id.clone(), self.id))
    }

    pub fn partitions_from_ids<'a>(
        &'a self,
        partition_ids: &'a [PartitionId],
    ) -> Result<impl Iterator<Item = (&'a PartitionId, &'a Partition)>, Error> {
        if partition_ids.is_empty() {
            Ok(Either::Left(self.partitions.iter()))
        } else {
            // Validate that all partition IDs exist
            for id in partition_ids {
                if !self.partitions.contains_key(id) {
                    return Err(Error::PartitionIdNotFound(id.clone(), self.id));
                }
            }

            Ok(Either::Right(partition_ids.iter().filter_map(|id| {
                self.partitions.get(id).map(|partition| (id, partition))
            })))
        }
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

    pub fn iter_layers(&self) -> impl Iterator<Item = &Layer> {
        self.partitions
            .values()
            .flat_map(|partition| partition.iter_layers().map(|(_, layer)| layer))
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        Schema::try_merge(self.iter_layers().map(|layer| layer.schema()))
    }

    pub fn partition_ids(&self) -> impl Iterator<Item = PartitionId> {
        self.partitions.keys().cloned()
    }

    //TODO(RR-2604): add support for property columns
    pub fn partition_table(&self) -> arrow::error::Result<RecordBatch> {
        let (partition_ids, last_updated_at, num_chunks, size_bytes, layer_names, storage_urls): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = itertools::multiunzip(self.partitions.iter().map(|(partition_id, partition)| {
            let (layer_names, storage_urls): (Vec<_>, Vec<_>) =
                itertools::multiunzip(partition.iter_layers().map(|(layer_name, _)| {
                    (
                        layer_name.to_owned(),
                        format!("memory:///{}/{partition_id}/{layer_name}", self.id),
                    )
                }));

            (
                partition_id.to_string(),
                partition.last_updated_at().as_nanosecond() as i64,
                partition.num_chunks(),
                partition.size_bytes(),
                layer_names,
                storage_urls,
            )
        }));

        ScanPartitionTableResponse::create_dataframe(
            partition_ids,
            layer_names,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
    }

    pub fn dataset_manifest(&self) -> arrow::error::Result<RecordBatch> {
        let (
            layer_names,
            partition_ids,
            storage_urls,
            layer_types,
            registration_times,
            last_updated_at,
            num_chunks,
            size_bytes,
            schema_sha256s,
        ): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = itertools::process_results(
            self.partitions
                .iter()
                .flat_map(|(partition_id, partition)| {
                    let partition_id = partition_id.to_string();
                    partition.iter_layers().map(
                        move |(layer_name, layer)| -> Result<_, ArrowError> {
                            Ok((
                                layer_name.to_owned(),
                                partition_id.clone(),
                                format!("memory:///{}/{partition_id}/{layer_name}", self.id),
                                layer.layer_type().to_owned(),
                                layer.registration_time().as_nanosecond() as i64,
                                layer.last_updated_at().as_nanosecond() as i64,
                                layer.num_chunks(),
                                layer.size_bytes(),
                                layer.schema_sha256()?,
                            ))
                        },
                    )
                }),
            |iter| itertools::multiunzip(iter),
        )?;

        ScanDatasetManifestResponse::create_dataframe(
            layer_names,
            partition_ids,
            storage_urls,
            layer_types,
            registration_times,
            last_updated_at,
            num_chunks,
            size_bytes,
            schema_sha256s,
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
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        re_log::debug!(?partition_id, ?layer_name, "add_layer");

        self.partitions
            .entry(partition_id)
            .or_default()
            .insert_layer(layer_name, Layer::new(store_handle), on_duplicate)?;

        self.updated_at = jiff::Timestamp::now();
        Ok(())
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

            self.add_layer(
                partition_id.clone(),
                layer_name.to_owned(),
                chunk_store,
                on_duplicate,
            )?;

            new_partition_ids.insert(partition_id);
        }

        Ok(new_partition_ids)
    }
}
