use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Fields, Schema};
use itertools::Either;
use re_arrow_util::RecordBatchExt as _;
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

    /// Returns the partitions from the given list of id.
    ///
    /// As per our proto conventions, all partitions are returned if none is listed.
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

    pub fn partition_table(&self) -> Result<RecordBatch, Error> {
        let row_count = self.partitions.len();

        let mut all_partition_properties = Vec::with_capacity(row_count);

        let mut partition_ids = Vec::with_capacity(row_count);
        let mut layer_names = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);

        for (partition_id, partition) in &self.partitions {
            let layer_count = partition.layer_count();
            let mut layer_names_row = Vec::with_capacity(layer_count);
            let mut storage_urls_row = Vec::with_capacity(layer_count);

            let mut current_partition_properties = BTreeMap::default();

            for (layer_name, layer) in partition.iter_layers() {
                layer_names_row.push(layer_name.to_owned());
                storage_urls_row.push(format!("memory:///{}/{partition_id}/{layer_name}", self.id));

                let layer_properties = layer
                    .compute_properties()
                    .map_err(Error::failed_to_extract_properties)?;

                // Accumulate properties.
                //
                // The semantics for the layer to partition property propagation is that the
                // last registered layer wins. The code below achieves this by virtual of the
                // layers being iterated in registration order.
                for (col_idx, field) in layer_properties.schema().fields().iter().enumerate() {
                    current_partition_properties.insert(
                        Arc::clone(field),
                        Arc::clone(layer_properties.column(col_idx)),
                    );
                }
            }

            let properties_batch = RecordBatch::try_new_with_options(
                Arc::new(Schema::new_with_metadata(
                    current_partition_properties
                        .keys()
                        .map(Arc::clone)
                        .collect::<Fields>(),
                    Default::default(),
                )),
                current_partition_properties.into_values().collect(),
                // There should always be exactly one row, one per partition. Also, we must specify
                // it anyway for the cases where there are no properties at all (so arrow is unable
                // to infer the row count).
                &RecordBatchOptions::default().with_row_count(Some(1)),
            )
            .map_err(Error::failed_to_extract_properties)?;

            all_partition_properties.push(properties_batch);

            partition_ids.push(partition_id.to_string());
            layer_names.push(layer_names_row);
            storage_urls.push(storage_urls_row);
            last_updated_at.push(partition.last_updated_at().as_nanosecond() as i64);
            num_chunks.push(partition.num_chunks());
            size_bytes.push(partition.size_bytes());
        }

        let properties_record_batch =
            re_arrow_util::concat_polymorphic_batches(all_partition_properties.as_slice())
                .map_err(Error::failed_to_extract_properties)?;

        let base_record_batch = ScanPartitionTableResponse::create_dataframe(
            partition_ids,
            layer_names,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
        .map_err(Error::failed_to_extract_properties)?;

        base_record_batch
            .concat_horizontally_with(&properties_record_batch)
            .map_err(Error::failed_to_extract_properties)
    }

    pub fn dataset_manifest(&self) -> Result<RecordBatch, Error> {
        let row_count = self.partitions.values().map(|p| p.layer_count()).sum();
        let mut layer_names = Vec::with_capacity(row_count);
        let mut partition_ids = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut layer_types = Vec::with_capacity(row_count);
        let mut registration_times = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);
        let mut schema_sha256s = Vec::with_capacity(row_count);

        let mut properties = Vec::with_capacity(row_count);

        for (layer_name, partition_id, layer) in
            self.partitions
                .iter()
                .flat_map(|(partition_id, partition)| {
                    let partition_id = partition_id.to_string();
                    partition
                        .iter_layers()
                        .map(move |(layer_name, layer)| (layer_name, partition_id.clone(), layer))
                })
        {
            layer_names.push(layer_name.to_owned());
            storage_urls.push(format!("memory:///{}/{partition_id}/{layer_name}", self.id));
            partition_ids.push(partition_id);
            layer_types.push(layer.layer_type().to_owned());
            registration_times.push(layer.registration_time().as_nanosecond() as i64);
            last_updated_at.push(layer.last_updated_at().as_nanosecond() as i64);
            num_chunks.push(layer.num_chunks());
            size_bytes.push(layer.size_bytes());
            schema_sha256s.push(
                layer
                    .schema_sha256()
                    .map_err(Error::failed_to_extract_properties)?,
            );

            properties.push(
                layer
                    .compute_properties()
                    .map_err(Error::failed_to_extract_properties)?,
            );
        }

        let base_record_batch = ScanDatasetManifestResponse::create_dataframe(
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
        .map_err(Error::failed_to_extract_properties)?;

        let properties_record_batch =
            re_arrow_util::concat_polymorphic_batches(properties.as_slice())
                .map_err(Error::failed_to_extract_properties)?;

        base_record_batch
            .concat_horizontally_with(&properties_record_batch)
            .map_err(Error::failed_to_extract_properties)
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
