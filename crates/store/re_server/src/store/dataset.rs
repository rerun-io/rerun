use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, create_array};
use arrow::datatypes::{Field, Fields, Schema};
use itertools::{Either, Itertools as _};
use parking_lot::Mutex;
use re_arrow_util::RecordBatchExt as _;
use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_log_encoding::RawRrdManifest;
use re_log_types::{EntryId, StoreId, StoreKind, TimeType};
use re_protos::cloud::v1alpha1::ext::{DataSource, DatasetDetails, DatasetEntry, EntryDetails};
use re_protos::cloud::v1alpha1::{
    EntryKind, ScanDatasetManifestResponse, ScanSegmentTableResponse,
};
use re_protos::common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, SegmentId};

use crate::store::{Error, InMemoryStore, Layer, Segment, Tracked};

/// The mutable inner state of a [`Dataset`], wrapped in [`Tracked`] for automatic timestamp updates.
pub struct DatasetInner {
    name: String,
    details: DatasetDetails,
    segments: HashMap<SegmentId, Segment>,
    #[cfg(feature = "lance")]
    indexes: crate::chunk_index::DatasetChunkIndexes,
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
                segments: HashMap::default(),
                #[cfg(feature = "lance")]
                indexes: crate::chunk_index::DatasetChunkIndexes::new(id),
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

    #[cfg(feature = "lance")]
    pub fn indexes(&self) -> &crate::chunk_index::DatasetChunkIndexes {
        &self.inner.indexes
    }

    pub fn segments(&self) -> &HashMap<SegmentId, Segment> {
        &self.inner.segments
    }

    pub fn segment(&self, segment_id: &SegmentId) -> Result<&Segment, Error> {
        self.inner
            .segments
            .get(segment_id)
            .ok_or_else(|| Error::SegmentIdNotFound(segment_id.clone(), self.id))
    }

    /// Returns the segments from the given list of id.
    ///
    /// As per our proto conventions, all segments are returned if none is listed.
    pub fn segments_from_ids<'a>(
        &'a self,
        segment_ids: &'a [SegmentId],
    ) -> Result<impl Iterator<Item = (&'a SegmentId, &'a Segment)>, Error> {
        if segment_ids.is_empty() {
            Ok(Either::Left(self.inner.segments.iter()))
        } else {
            // Validate that all segment IDs exist
            for id in segment_ids {
                if !self.inner.segments.contains_key(id) {
                    return Err(Error::SegmentIdNotFound(id.clone(), self.id));
                }
            }

            Ok(Either::Right(segment_ids.iter().filter_map(|id| {
                self.inner.segments.get(id).map(|segment| (id, segment))
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
            .segments
            .values()
            .flat_map(|segment| segment.iter_layers().map(|(_, layer)| layer))
    }

    // TODO(ab): now that we systematically check the merged schema upon registration, we could
    // switch to keeping around a fully merged dataset schema instead of the present caching
    // strategy. (That is, if performance requires it.)
    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let mut cache = self.cached_schema.lock();

        let updated_at = self.updated_at();

        // Check if we have a valid cached schema
        if let Some((cached_at, schema)) = cache.as_ref()
            && *cached_at == updated_at
        {
            return Ok(Schema::clone(schema));
        }

        // Recompute schema
        let schema = Schema::try_merge(self.iter_layers().map(|layer| layer.schema()))?;
        let schema_arc = Arc::new(schema.clone());
        *cache = Some((updated_at, Arc::clone(&schema_arc)));

        Ok(schema)
    }

    pub fn segment_ids(&self) -> impl Iterator<Item = SegmentId> {
        self.inner.segments.keys().cloned()
    }

    pub fn segment_table(&self) -> Result<RecordBatch, Error> {
        let row_count = self.inner.segments.len();

        let mut all_segment_properties = Vec::with_capacity(row_count);

        let mut segment_ids = Vec::with_capacity(row_count);
        let mut layer_names = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);

        let mut all_index_ranges = Vec::with_capacity(row_count);

        for (segment_id, segment) in &self.inner.segments {
            let layer_count = segment.layer_count();
            let mut layer_names_row = Vec::with_capacity(layer_count);
            let mut storage_urls_row = Vec::with_capacity(layer_count);

            let mut current_segment_properties = BTreeMap::default();
            let mut current_segment_indexes = BTreeMap::default();

            for (layer_name, layer) in segment.iter_layers() {
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

                for (time_name, range) in layer.index_ranges() {
                    let entry = current_segment_indexes.entry(time_name).or_insert(range);
                    *entry = entry.union(range);
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

            let indexes_batch = RecordBatch::try_new_with_options(
                Arc::new(Schema::new_with_metadata(
                    current_segment_indexes
                        .keys()
                        .flat_map(|timeline| {
                            ["end", "start"].into_iter().map(|index_marker| {
                                let metadata: HashMap<_, _> = [
                                    ("rerun:index".to_owned(), timeline.name().to_string()),
                                    ("rerun:index_kind".to_owned(), timeline.typ().to_string()),
                                    ("rerun:index_marker".to_owned(), index_marker.to_owned()),
                                    ("rerun:kind".to_owned(), "index".to_owned()),
                                ]
                                .into_iter()
                                .collect();
                                let field_name = format!("{}:{index_marker}", timeline.name());
                                let data_type = timeline.datatype();
                                Arc::new(
                                    Field::new(field_name, data_type, true).with_metadata(metadata),
                                )
                            })
                        })
                        .collect_vec(),
                    HashMap::default(),
                )),
                current_segment_indexes
                    .into_iter()
                    .flat_map(|(timeline, range)| match timeline.typ() {
                        TimeType::Sequence => [
                            create_array!(Int64, [range.max().as_i64()]) as ArrayRef,
                            create_array!(Int64, [range.min().as_i64()]) as ArrayRef,
                        ],
                        TimeType::DurationNs => [
                            create_array!(DurationNanosecond, [range.max().as_i64()]) as ArrayRef,
                            create_array!(DurationNanosecond, [range.min().as_i64()]) as ArrayRef,
                        ],
                        TimeType::TimestampNs => [
                            create_array!(TimestampNanosecond, [range.max().as_i64()]) as ArrayRef,
                            create_array!(TimestampNanosecond, [range.min().as_i64()]) as ArrayRef,
                        ],
                    })
                    .collect(),
                &RecordBatchOptions::default().with_row_count(Some(1)),
            )?;

            all_segment_properties.push(properties_batch);
            all_index_ranges.push(indexes_batch);

            segment_ids.push(segment_id.to_string());
            layer_names.push(layer_names_row);
            storage_urls.push(storage_urls_row);
            last_updated_at.push(segment.last_updated_at().as_nanosecond() as i64);
            num_chunks.push(segment.num_chunks());
            size_bytes.push(segment.size_bytes());
        }

        let properties_record_batch =
            re_arrow_util::concat_polymorphic_batches(all_segment_properties.as_slice())
                .map_err(Error::failed_to_extract_properties)?;
        let indexes_record_batch =
            re_arrow_util::concat_polymorphic_batches(all_index_ranges.as_slice())?;

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
            .map_err(Error::failed_to_extract_properties)?
            .concat_horizontally_with(&indexes_record_batch)
            .map_err(Into::into)
    }

    pub fn dataset_manifest(&self) -> Result<RecordBatch, Error> {
        let row_count = self.inner.segments.values().map(|s| s.layer_count()).sum();

        let mut layer_names = Vec::with_capacity(row_count);
        let mut segment_ids = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut layer_types = Vec::with_capacity(row_count);
        let mut registration_times = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);
        let mut schema_sha256s = Vec::with_capacity(row_count);

        let mut properties = Vec::with_capacity(row_count);

        for (layer_name, segment_id, layer) in
            self.inner
                .segments
                .iter()
                .flat_map(|(segment_id, segment)| {
                    let segment_id = segment_id.to_string();
                    segment
                        .iter_layers()
                        .map(move |(layer_name, layer)| (layer_name, segment_id.clone(), layer))
                })
        {
            layer_names.push(layer_name.to_owned());
            storage_urls.push(format!("memory:///{}/{segment_id}/{layer_name}", self.id));
            segment_ids.push(segment_id);
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
            segment_ids,
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

    pub fn rrd_manifest(&self, segment_id: &SegmentId) -> Result<RawRrdManifest, Error> {
        let partition = self.segment(segment_id)?;

        let mut rrd_manifest_builder = re_log_encoding::RrdManifestBuilder::default();

        let mut chunk_keys = Vec::new();

        for (layer_name, layer) in partition.iter_layers() {
            let store = layer.store_handle();

            let mut offset = 0;
            for chunk in store.read().iter_physical_chunks() {
                let chunk_batch = chunk
                    .to_chunk_batch()
                    .map_err(|err| Error::RrdLoadingError(err.into()))?;

                // Not a totally accurate value, but we're certainly not going to encode every chunk
                // into IPC bytes just to figure out their uncompressed size either.
                //
                // This is fine for 2 reasons:
                // 1. The reported size is mostly for human and automated heuristics (e.g. "have I
                //    enough memory left to download this chunk?"), and so doesn't need to be exact.
                // 2. Reporting the size in terms of heap values is even better for such heuristics.
                use re_byte_size::SizeBytes as _;
                let byte_size_uncompressed = chunk.heap_size_bytes();

                // There is no such thing as "compressed data on disk" in the case of the OSS server,
                // since there's no disk to begin with. That's fine, we just re-use the
                // uncompressed values: the chunk-key (generated below) is what will be used to
                // accurately fetch the data in any case.
                //
                // TODO(cmc): we could also keep track of the compressed values originally fetched
                // from disk and/or network all the way into the OSS server's datastructures and
                // resurface them here but that doesn't seem to have any practical use, so not
                // worth the added complexity?
                let uncompressed_byte_span = re_span::Span {
                    start: offset,
                    len: byte_size_uncompressed,
                };

                offset += byte_size_uncompressed;

                rrd_manifest_builder
                    .append(&chunk_batch, uncompressed_byte_span, byte_size_uncompressed)
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

        let application_id = "n/a"; // irrelevant, dropped immediately
        let store_id = StoreId::new(self.store_kind(), application_id, segment_id.to_string());
        let mut rrd_manifest = rrd_manifest_builder
            .build(store_id)
            .map_err(|err| Error::RrdLoadingError(err.into()))?;

        {
            let (schema, mut columns, num_rows) = rrd_manifest.data.clone().into_parts();

            let schema = {
                let mut schema = Arc::unwrap_or_clone(schema);
                let mut fields = schema.fields.to_vec();
                fields.push(Arc::new(RawRrdManifest::field_chunk_key()));
                schema.fields = fields.into();
                schema
            };
            {
                let chunk_keys = arrow::array::BinaryArray::from_iter_values(chunk_keys.iter());
                columns.push(Arc::new(chunk_keys));
            }

            rrd_manifest.data = RecordBatch::try_new_with_options(
                Arc::new(schema),
                columns,
                &RecordBatchOptions::new().with_row_count(Some(num_rows)),
            )?;
        }

        Ok(rrd_manifest)
    }

    pub fn layer_store_handle(
        &self,
        segment_id: &SegmentId,
        layer_name: &str,
    ) -> Option<&ChunkStoreHandle> {
        self.inner
            .segments
            .get(segment_id)
            .and_then(|segment| segment.layer(layer_name))
            .map(|layer| layer.store_handle())
    }

    // we can't expect there are no async calls without the lance feature
    #[allow(clippy::allow_attributes)]
    #[allow(clippy::unused_async)]
    pub async fn add_layer(
        &mut self,
        segment_id: SegmentId,
        layer_name: String,
        store_handle: ChunkStoreHandle,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        re_log::debug!(?segment_id, ?layer_name, "add_layer");

        // Validate schema compatibility before inserting
        let current_schema = self.schema()?;
        let new_layer_schema = {
            let fields = store_handle.read().schema().arrow_fields();
            Schema::new_with_metadata(fields, HashMap::default())
        };
        Schema::try_merge([current_schema, new_layer_schema]).map_err(|err| {
            Error::SchemaConflict(format!(
                "schema incompatibility on segment '{segment_id}', layer '{layer_name}': {err}"
            ))
        })?;

        let overwritten = self
            .inner
            .modify()
            .segments
            .entry(segment_id.clone())
            .or_default()
            .insert_layer(
                layer_name.clone(),
                Layer::new(store_handle.clone()),
                on_duplicate,
            )?;

        #[cfg(feature = "lance")]
        self.indexes()
            .on_layer_added(segment_id, store_handle, &layer_name, overwritten)
            .await?;

        #[cfg(not(feature = "lance"))]
        let _ = overwritten;

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

        let mut new_segment_ids = BTreeSet::default();

        for (store_id, chunk_store) in contents {
            if store_id.kind() != store_kind {
                continue;
            }

            let segment_id = SegmentId::new(store_id.recording_id().to_string());

            self.add_layer(
                segment_id.clone(),
                layer_name.to_owned(),
                chunk_store.clone(),
                on_duplicate,
            )
            .await?;

            new_segment_ids.insert(segment_id.clone());
        }

        Ok(new_segment_ids)
    }
}
