use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Fields, Schema};
use itertools::Either;
use parking_lot::Mutex;
use re_arrow_util::{RecordBatchExt as _, RecordBatchTestExt as _};
use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_log_types::{EntryId, StoreKind};
use re_protos::cloud::v1alpha1::ext::{DataSource, DatasetDetails, DatasetEntry, EntryDetails};
use re_protos::cloud::v1alpha1::{
    EntryKind, ScanDatasetManifestResponse, ScanSegmentTableResponse,
};
use re_protos::common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, SegmentId};

use crate::chunk_index::DatasetChunkIndexes;
use crate::store::{Error, InMemoryStore, Layer, Partition, Tracked};

/// The mutable inner state of a [`Dataset`], wrapped in [`Tracked`] for automatic timestamp updates.
pub struct DatasetInner {
    name: String,
    details: DatasetDetails,
    partitions: HashMap<SegmentId, Partition>,
    indexes: DatasetChunkIndexes,
}

pub struct Dataset {
    id: EntryId,
    store_kind: StoreKind,
    created_at: jiff::Timestamp,
    inner: Tracked<DatasetInner>,

    /// Cached schema with the timestamp when it was computed.
    /// Invalidated when `updated_at` changes.
    cached_schema: Mutex<Option<(jiff::Timestamp, Arc<Schema>)>>,
}

impl Dataset {
    pub fn new(id: EntryId, name: String, store_kind: StoreKind, details: DatasetDetails) -> Self {
        Self {
            id,
            store_kind,
            created_at: jiff::Timestamp::now(),
            inner: Tracked::new(DatasetInner {
                name,
                details,
                partitions: HashMap::default(),
                indexes: DatasetChunkIndexes::new(id),
            }),
            cached_schema: Mutex::new(None),
        }
    }

    #[inline]
    pub fn id(&self) -> EntryId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn set_name(&mut self, name: String) {
        if name != self.inner.name {
            self.inner.modify().name = name;
        }
    }

    #[inline]
    pub fn store_kind(&self) -> StoreKind {
        self.store_kind
    }

    #[inline]
    pub fn entry_kind(&self) -> EntryKind {
        match self.store_kind() {
            StoreKind::Recording => EntryKind::Dataset,
            StoreKind::Blueprint => EntryKind::BlueprintDataset,
        }
    }

    #[inline]
    pub fn updated_at(&self) -> jiff::Timestamp {
        self.inner.updated_at()
    }

    pub fn indexes(&self) -> &DatasetChunkIndexes {
        &self.inner.indexes
    }

    pub fn partitions(&self) -> &HashMap<SegmentId, Partition> {
        &self.inner.partitions
    }

    pub fn partition(&self, segment_id: &SegmentId) -> Result<&Partition, Error> {
        self.inner
            .partitions
            .get(segment_id)
            .ok_or_else(|| Error::SegmentIdNotFound(segment_id.clone(), self.id))
    }

    /// Returns the partitions from the given list of id.
    ///
    /// As per our proto conventions, all partitions are returned if none is listed.
    pub fn partitions_from_ids<'a>(
        &'a self,
        segment_ids: &'a [SegmentId],
    ) -> Result<impl Iterator<Item = (&'a SegmentId, &'a Partition)>, Error> {
        if segment_ids.is_empty() {
            Ok(Either::Left(self.inner.partitions.iter()))
        } else {
            // Validate that all segment IDs exist
            for id in segment_ids {
                if !self.inner.partitions.contains_key(id) {
                    return Err(Error::SegmentIdNotFound(id.clone(), self.id));
                }
            }

            Ok(Either::Right(segment_ids.iter().filter_map(|id| {
                self.inner
                    .partitions
                    .get(id)
                    .map(|partition| (id, partition))
            })))
        }
    }

    pub fn dataset_details(&self) -> &DatasetDetails {
        &self.inner.details
    }

    pub fn set_dataset_details(&mut self, details: DatasetDetails) {
        if details != self.inner.details {
            self.inner.modify().details = details;
        }
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.inner.name.clone(),
            kind: self.entry_kind(),
            created_at: self.created_at,
            updated_at: self.inner.updated_at(),
        }
    }

    pub fn as_dataset_entry(&self) -> DatasetEntry {
        DatasetEntry {
            details: EntryDetails {
                id: self.id,
                name: self.inner.name.clone(),
                kind: self.entry_kind(),
                created_at: self.created_at,
                updated_at: self.inner.updated_at(),
            },

            dataset_details: self.inner.details.clone(),

            handle: DatasetHandle {
                id: Some(self.id),
                store_kind: self.store_kind,
                url: url::Url::parse(&format!("memory:///{}", self.id)).expect("valid url"),
            },
        }
    }

    pub fn iter_layers(&self) -> impl Iterator<Item = &Layer> {
        self.inner
            .partitions
            .values()
            .flat_map(|partition| partition.iter_layers().map(|(_, layer)| layer))
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let mut cache = self.cached_schema.lock();

        let updated_at = self.updated_at();

        // Check if we have a valid cached schema
        if let Some((cached_at, schema)) = cache.as_ref() {
            if *cached_at == updated_at {
                return Ok(Schema::clone(schema));
            }
        }

        // Recompute schema
        let schema = Schema::try_merge(self.iter_layers().map(|layer| layer.schema()))?;
        let schema_arc = Arc::new(schema.clone());
        *cache = Some((updated_at, Arc::clone(&schema_arc)));

        Ok(schema)
    }

    pub fn segment_ids(&self) -> impl Iterator<Item = SegmentId> {
        self.inner.partitions.keys().cloned()
    }

    pub fn segment_table(&self) -> Result<RecordBatch, Error> {
        let row_count = self.inner.partitions.len();

        let mut all_segment_properties = Vec::with_capacity(row_count);

        let mut segment_ids = Vec::with_capacity(row_count);
        let mut layer_names = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);

        for (segment_id, partition) in &self.inner.partitions {
            let layer_count = partition.layer_count();
            let mut layer_names_row = Vec::with_capacity(layer_count);
            let mut storage_urls_row = Vec::with_capacity(layer_count);

            let mut current_segment_properties = BTreeMap::default();

            for (layer_name, layer) in partition.iter_layers() {
                layer_names_row.push(layer_name.to_owned());
                storage_urls_row.push(format!("memory:///{}/{segment_id}/{layer_name}", self.id));

                let layer_properties = layer
                    .compute_properties()
                    .map_err(Error::failed_to_extract_properties)?;

                // Accumulate properties.
                //
                // The semantics for the layer to segment property propagation is that the
                // last registered layer wins. The code below achieves this by virtual of the
                // layers being iterated in registration order.
                for (col_idx, field) in layer_properties.schema().fields().iter().enumerate() {
                    current_segment_properties.insert(
                        Arc::clone(field),
                        Arc::clone(layer_properties.column(col_idx)),
                    );
                }
            }

            let properties_batch = RecordBatch::try_new_with_options(
                Arc::new(Schema::new_with_metadata(
                    current_segment_properties
                        .keys()
                        .map(Arc::clone)
                        .collect::<Fields>(),
                    Default::default(),
                )),
                current_segment_properties.into_values().collect(),
                // There should always be exactly one row, one per segment. Also, we must specify
                // it anyway for the cases where there are no properties at all (so arrow is unable
                // to infer the row count).
                &RecordBatchOptions::default().with_row_count(Some(1)),
            )
            .map_err(Error::failed_to_extract_properties)?;

            all_segment_properties.push(properties_batch);

            segment_ids.push(segment_id.to_string());
            layer_names.push(layer_names_row);
            storage_urls.push(storage_urls_row);
            last_updated_at.push(partition.last_updated_at().as_nanosecond() as i64);
            num_chunks.push(partition.num_chunks());
            size_bytes.push(partition.size_bytes());
        }

        let properties_record_batch =
            re_arrow_util::concat_polymorphic_batches(all_segment_properties.as_slice())
                .map_err(Error::failed_to_extract_properties)?;

        let base_record_batch = ScanSegmentTableResponse::create_dataframe(
            segment_ids,
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
        let row_count = self
            .inner
            .partitions
            .values()
            .map(|p| p.layer_count())
            .sum();

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
            self.inner
                .partitions
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

    pub fn rrd_manifest(&self, segment_id: &SegmentId) -> Result<RecordBatch, Error> {
        let partition = self.partition(segment_id)?;

        let mut rrd_manifest_builder = re_log_encoding::RrdManifestBuilder::default();

        let mut chunk_keys = Vec::new();

        for (layer_name, layer) in partition.iter_layers() {
            let store = layer.store_handle();

            for chunk in store.read().iter_chunks() {
                let chunk_batch = chunk
                    .to_chunk_batch()
                    .map_err(|err| Error::RrdLoadingError(err.into()))?;

                // TODO(RR-3110): add an alternate RrdManifestBuilder builder with chunk keys instead of byte spans.
                let dummy_byte_span = re_span::Span::default();
                rrd_manifest_builder
                    .append(&chunk_batch, dummy_byte_span)
                    .map_err(|err| Error::RrdLoadingError(err.into()))?;

                chunk_keys.push(
                    crate::store::ChunkKey {
                        chunk_id: chunk.id(),
                        segment_id: segment_id.clone(),
                        layer_name: layer_name.to_owned(),
                        dataset_id: self.id(),
                    }
                    .encode()?,
                );
            }
        }

        let rrd_manifest_batch = rrd_manifest_builder
            .into_record_batch()
            .map_err(|err| Error::RrdLoadingError(err.into()))?
            .remove_columns(&["chunk_byte_offset", "chunk_byte_size"]);

        let (schema, mut columns, num_rows) = rrd_manifest_batch.into_parts();

        let schema = {
            let mut schema = Arc::unwrap_or_clone(schema);
            let mut fields = schema.fields.to_vec();
            fields.push(Arc::new(arrow::datatypes::Field::new(
                "chunk_key",
                arrow::datatypes::DataType::Binary,
                false,
            )));
            schema.fields = fields.into();
            schema
        };
        {
            let chunk_keys = arrow::array::BinaryArray::from_iter_values(chunk_keys.iter());
            columns.push(Arc::new(chunk_keys));
        }

        let rrd_manifest_batch = RecordBatch::try_new_with_options(
            Arc::new(schema),
            columns,
            &RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )?;

        Ok(rrd_manifest_batch)
    }

    pub fn layer_store_handle(
        &self,
        segment_id: &SegmentId,
        layer_name: &str,
    ) -> Option<&ChunkStoreHandle> {
        self.inner
            .partitions
            .get(segment_id)
            .and_then(|partition| partition.layer(layer_name))
            .map(|layer| layer.store_handle())
    }

    pub async fn add_layer(
        &mut self,
        segment_id: SegmentId,
        layer_name: String,
        store_handle: ChunkStoreHandle,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        re_log::debug!(?segment_id, ?layer_name, "add_layer");

        let overwritten = self
            .inner
            .modify()
            .partitions
            .entry(segment_id.clone())
            .or_default()
            .insert_layer(
                layer_name.clone(),
                Layer::new(store_handle.clone()),
                on_duplicate,
            )?;

        self.indexes()
            .on_layer_added(segment_id, store_handle, &layer_name, overwritten)
            .await?;

        Ok(())
    }

    /// Load a RRD using its recording id as segment id.
    ///
    /// Only stores with matching kinds with be loaded.
    pub async fn load_rrd(
        &mut self,
        path: &Path,
        layer_name: Option<&str>,
        on_duplicate: IfDuplicateBehavior,
        store_kind: StoreKind,
    ) -> Result<BTreeSet<SegmentId>, Error> {
        re_log::info!("Loading RRD: {}", path.display());
        let contents =
            ChunkStore::handle_from_rrd_filepath(&InMemoryStore::chunk_store_config(), path)
                .map_err(Error::RrdLoadingError)?;

        let layer_name = layer_name.unwrap_or(DataSource::DEFAULT_LAYER);

        let mut new_partition_ids = BTreeSet::default();

        for (store_id, chunk_store) in contents {
            if store_id.kind() != store_kind {
                continue;
            }

            let partition_id = SegmentId::new(store_id.recording_id().to_string());

            self.add_layer(
                partition_id.clone(),
                layer_name.to_owned(),
                chunk_store.clone(),
                on_duplicate,
            )
            .await?;

            new_partition_ids.insert(partition_id.clone());
        }

        Ok(new_partition_ids)
    }
}
