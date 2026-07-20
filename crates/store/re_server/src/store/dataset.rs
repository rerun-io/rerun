#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeSet;
use std::collections::{BTreeMap, HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, RecordBatch, RecordBatchOptions, create_array};
use arrow::datatypes::{Field, Fields, Schema};
use itertools::{Either, Itertools as _};
use parking_lot::Mutex;
use re_arrow_util::RecordBatchExt as _;
use re_log_encoding::RawRrdManifest;
use re_log_types::{EntryId, StoreId, StoreKind, TimeType};
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext as cloud_ext;
use re_protos::cloud::v1alpha1::ext::ScanDatasetManifestDataframe;
use re_protos::cloud::v1alpha1::ext::{DataSourceKind, DatasetDetails, DatasetEntry, EntryDetails};
use re_protos::cloud::v1alpha1::{EntryKind, ScanSegmentTableResponse};
use re_protos::common::v1alpha1::ext::{
    DatasetHandle, DatasetKind, IfDuplicateBehavior, SegmentId,
};
use re_types_core::LayerName;

#[cfg(not(target_arch = "wasm32"))]
use crate::store::store_pool::StorePool;
use crate::store::{
    Error, LayerInfo, ResolvedStore, Segment, Source, SourceInsertOutcome, StoreSlotId, Tracked,
};

/// The mutable inner state of a [`Dataset`], wrapped in [`Tracked`] for automatic timestamp updates.
pub struct DatasetInner {
    name: EntryName,

    details: DatasetDetails,

    segments: HashMap<SegmentId, Segment>,
}

pub struct Dataset {
    id: EntryId,
    dataset_kind: DatasetKind,
    created_at: jiff::Timestamp,
    inner: Tracked<DatasetInner>,

    /// Cached schema with the timestamp when it was computed.
    /// Invalidated when `updated_at` changes.
    cached_schema: Mutex<Option<(jiff::Timestamp, Arc<Schema>)>>,
}

impl Dataset {
    pub fn new(
        id: EntryId,
        name: EntryName,
        dataset_kind: DatasetKind,
        details: DatasetDetails,
    ) -> Self {
        Self {
            id,
            dataset_kind,
            created_at: jiff::Timestamp::now(),
            inner: Tracked::new(DatasetInner {
                name,
                details,
                segments: Default::default(),
            }),
            cached_schema: Mutex::new(None),
        }
    }

    #[inline]
    pub fn id(&self) -> EntryId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &EntryName {
        &self.inner.name
    }

    pub fn set_name(&mut self, name: EntryName) {
        if name != self.inner.name {
            self.inner.modify().name = name;
        }
    }

    #[inline]
    pub fn dataset_kind(&self) -> DatasetKind {
        self.dataset_kind
    }

    #[inline]
    pub fn store_kind(&self) -> StoreKind {
        self.dataset_kind.store_kind()
    }

    #[inline]
    pub fn entry_kind(&self) -> EntryKind {
        match self.dataset_kind {
            DatasetKind::Recording => EntryKind::Dataset,
            DatasetKind::Blueprint => EntryKind::BlueprintDataset,
            DatasetKind::Asset => EntryKind::AssetDataset,
        }
    }

    #[inline]
    pub fn updated_at(&self) -> jiff::Timestamp {
        self.inner.updated_at()
    }

    pub fn segments(&self) -> &HashMap<SegmentId, Segment> {
        &self.inner.segments
    }

    pub fn segment(&self, segment_id: &SegmentId) -> Result<&Segment, Error> {
        self.inner
            .segments
            .get(segment_id)
            .ok_or_else(|| Error::SegmentIdNotFound {
                segment_id: segment_id.clone(),
                entry_id: self.id,
            })
    }

    /// Returns the segments from the given list of id.
    ///
    /// All segments are returned if `segment_ids` is `None`.
    ///
    /// Unknown segment IDs are silently skipped rather than treated as errors:
    /// callers (notably `QueryDataset`) may receive segment IDs from a DataFusion
    /// filter pushdown such as `WHERE rerun_segment_id = 'foo'`, where the value
    /// is data, not a referent. Erroring on a mismatch would turn ordinary SQL
    /// filters into hand-grenades. The same `segment_ids` field is also used by
    /// explicit API paths (e.g. `filter_segments`, `using_index_values`), which
    /// accept the same silent-ignore semantics in exchange for not paying a
    /// round-trip to validate IDs client-side.
    pub fn segments_from_ids<'a>(
        &'a self,
        segment_ids: Option<&'a [SegmentId]>,
    ) -> impl Iterator<Item = (&'a SegmentId, &'a Segment)> {
        if let Some(segment_ids) = segment_ids {
            Either::Left(
                segment_ids
                    .iter()
                    .filter_map(|id| self.inner.segments.get(id).map(|segment| (id, segment))),
            )
        } else {
            Either::Right(self.inner.segments.iter())
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
                dataset_kind: self.dataset_kind,
                url: url::Url::parse(&format!("memory:///{}", self.id)).expect("valid url"),
            },
        }
    }

    /// Iterate over all distinct sources of this dataset.
    pub fn iter_sources(&self) -> impl Iterator<Item = &Source> {
        self.inner
            .segments
            .values()
            .flat_map(|segment| segment.iter_sources().map(|(_, source)| source))
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
        let schema = Schema::try_merge(self.iter_sources().map(|source| source.schema()))?;
        let schema_arc = Arc::new(schema.clone());
        *cache = Some((updated_at, Arc::clone(&schema_arc)));

        Ok(schema)
    }

    pub fn segment_ids(&self) -> impl Iterator<Item = SegmentId> {
        self.inner.segments.keys().cloned()
    }

    pub async fn segment_table(&self) -> Result<RecordBatch, Error> {
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
            let layer_count = segment.source_count();
            let mut layer_names_row = Vec::with_capacity(layer_count);
            let mut storage_urls_row = Vec::with_capacity(layer_count);

            let mut current_segment_properties = BTreeMap::default();
            let mut current_segment_indexes = BTreeMap::default();

            for (layer_name, layer) in segment.iter_sources() {
                layer_names_row.push(layer_name.clone());
                storage_urls_row.push(format!("memory:///store/{}", layer.store_slot_id()));

                let layer_properties = layer.compute_properties().await?;

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

            segment_ids.push(segment_id.clone());
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

    pub async fn dataset_manifest(&self) -> Result<RecordBatch, Error> {
        self.dataset_manifest_filtered(None, None).await
    }

    /// Like [`Self::dataset_manifest`] but filtered down to just the segments/layers of interest.
    ///
    /// This method acts as a *product* filter:
    /// * `None` `segments_of_interest` + `None` `layers_of_interest`: everything
    /// * `None` `segments_of_interest` + `Some` `layers_of_interest`: return specified layers for *all* segments
    /// * `Some` `segments_of_interest` + `None` `layers_of_interest`: return *all* layers for specified segments
    /// * `Some` `segments_of_interest` + `Some` `layers_of_interest`: return *all* specified layers for *all* specified segments
    pub async fn dataset_manifest_filtered(
        &self,
        segments_of_interest: Option<&HashSet<&SegmentId>>,
        layers_of_interest: Option<&HashSet<&LayerName>>,
    ) -> Result<RecordBatch, Error> {
        let segment_rows = self
            .inner
            .segments
            .iter()
            .filter(|(segment_id, _)| {
                segments_of_interest.is_none_or(|segments| segments.contains(segment_id))
            })
            .flat_map(|(segment_id, segment)| {
                itertools::izip!(
                    std::iter::repeat(segment_id),
                    segment.iter_sources().filter(|(name, _layer)| {
                        layers_of_interest.is_none_or(|layers| layers.contains(name))
                    })
                )
            })
            .map(|(segment_id, (layer_name, source))| {
                let segment_id = segment_id.to_string();
                (layer_name, segment_id, source)
            });

        let layers: Vec<(&LayerName, String, &Source)> = segment_rows.collect();
        let row_count = layers.len();

        let mut layer_names = Vec::with_capacity(row_count);
        let mut segment_ids = Vec::with_capacity(row_count);
        let mut storage_urls = Vec::with_capacity(row_count);
        let mut layer_types = Vec::with_capacity(row_count);
        let mut registration_times = Vec::with_capacity(row_count);
        let mut last_updated_at = Vec::with_capacity(row_count);
        let mut num_chunks = Vec::with_capacity(row_count);
        let mut size_bytes = Vec::with_capacity(row_count);
        let mut schema_sha256s = Vec::with_capacity(row_count);
        let mut registration_statuses = Vec::with_capacity(row_count);

        let mut properties = Vec::with_capacity(row_count);

        for (layer_name, segment_id, source) in layers {
            layer_names.push(layer_name.clone());
            storage_urls.push(format!("memory:///store/{}", source.store_slot_id()));
            segment_ids.push(segment_id.into());
            layer_types.push(source.data_source_kind().to_string());
            registration_times.push(source.registration_time().as_nanosecond() as i64);
            last_updated_at.push(source.last_updated_at().as_nanosecond() as i64);
            num_chunks.push(source.num_chunks());
            size_bytes.push(source.size_bytes());
            schema_sha256s.push(
                source
                    .schema_sha256()
                    .map_err(Error::failed_to_extract_properties)?,
            );

            // In re_server, only successful registrations exist (schema conflicts fail synchronously),
            // so all entries are always `Done`.
            registration_statuses.push(cloud_ext::LayerRegistrationStatus::Done.to_string());

            properties.push(source.compute_properties().await?);
        }

        let base_record_batch = ScanDatasetManifestDataframe::new(
            layer_names,
            segment_ids,
            storage_urls,
            layer_types,
            registration_times,
            last_updated_at,
            num_chunks,
            size_bytes,
            schema_sha256s,
            registration_statuses,
        )
        .into_record_batch()
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
        let application_id = "n/a"; // irrelevant, dropped immediately
        let segment_store_id =
            StoreId::new(self.store_kind(), application_id, segment_id.to_string());

        // Each layer produces its own manifest (Lazy clones its cached footer, Eager rebuilds
        // from chunks), then we merge them under the segment-scoped store id.
        let per_layer: Vec<RawRrdManifest> = partition
            .iter_sources()
            .map(|(_, source)| source.rrd_manifest())
            .try_collect()?;

        RawRrdManifest::merge(segment_store_id, per_layer)
            .map_err(|err| Error::RrdLoadingError(err.into()))
    }

    /// Enforce this dataset kind's [registration limits](DatasetKind::limits) for a new source.
    ///
    /// Returns [`Error::SegmentLimitReached`] or [`Error::SegmentRejected`] if adding `source`
    /// under `segment_id` would exceed a limit. Recording and blueprint datasets are unlimited, so
    /// this is a no-op for them.
    fn enforce_limits(&self, segment_id: &SegmentId, source: &Source) -> Result<(), Error> {
        let limits = self.dataset_kind.limits();

        // Only new segments count against the limit. Adding a layer to an existing segment is fine.
        // Like the cloud server, the segment-count cap is enforced synchronously up front.
        if let Some(max) = limits.max_segment_count
            && !self.inner.segments.contains_key(segment_id)
            && self.inner.segments.len() as u64 >= max
        {
            return Err(Error::SegmentLimitReached(format!(
                "this {} already holds the maximum of {max} {}s",
                self.dataset_kind.name(),
                self.dataset_kind.contained_name(),
            )));
        }

        // The content checks below match the cloud server, which rejects them during the
        // registration task. `register_with_dataset` reports `SegmentRejected` as a failed task.
        if limits.static_chunks_only && source.has_temporal_chunks() {
            return Err(Error::SegmentRejected(format!(
                "{}s only accept static chunks, but {} '{segment_id}' contains temporal data",
                self.dataset_kind.name(),
                self.dataset_kind.contained_name(),
            )));
        }

        if let Some(max) = limits.max_segment_size_bytes {
            let existing = self
                .inner
                .segments
                .get(segment_id)
                .map_or(0, |segment| segment.size_bytes());
            let combined = existing + source.size_bytes();
            if combined > max {
                return Err(Error::SegmentRejected(format!(
                    "{} '{segment_id}' would be {combined} bytes, exceeding the {max}-byte limit for {}s",
                    self.dataset_kind.contained_name(),
                    self.dataset_kind.name(),
                )));
            }
        }

        Ok(())
    }

    // we can't expect there are no async calls without the lance feature
    #[allow(clippy::allow_attributes)]
    #[allow(clippy::unused_async)]
    pub async fn add_source(
        &mut self,
        segment_id: SegmentId,
        layer_info: Arc<LayerInfo>,
        store_slot_id: StoreSlotId,
        resolved: ResolvedStore,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        let layer_name = &layer_info.name;
        re_log::debug!(?segment_id, ?layer_name, "add_layer");

        // Validate schema compatibility before inserting.
        let current_schema = self.schema()?;
        let new_layer_schema = {
            let fields = resolved.schema().chunk_column_descriptors().arrow_fields();
            Schema::new_with_metadata(fields, HashMap::default())
        };
        for new_field in new_layer_schema.fields() {
            if let Ok(current_field) = current_schema.field_with_name(new_field.name())
                && current_field != new_field.as_ref()
            {
                re_arrow_util::reject_unsupported_widenings(new_field.data_type()).map_err(
                    |err| {
                        Error::SchemaConflict(format!(
                            "schema incompatibility on segment '{segment_id}', \
                             layer '{layer_name}': {err}"
                        ))
                    },
                )?;
            }
        }
        // Keep the merged schema so we can refresh the cache below.
        let merged_schema =
            Schema::try_merge([current_schema.clone(), new_layer_schema]).map_err(|err| {
                Error::SchemaConflict(format!(
                    "schema incompatibility on segment '{segment_id}', layer '{layer_name}': {err}"
                ))
            })?;

        let source = Arc::new(Source::new(
            store_slot_id,
            resolved,
            DataSourceKind::Rrd,
            layer_info,
        ));

        self.enforce_limits(&segment_id, &source)?;

        let outcome = self
            .inner
            .modify()
            .segments
            .entry(segment_id.clone())
            .or_default()
            .insert_source(source.clone(), on_duplicate)?;

        // Refresh the schema cache after each successful add_source to avoid
        // the O(N²) recompute pattern when register_with_dataset adds many
        // layers in a single batch. `self.inner.modify()` always bumps
        // `updated_at`, which would otherwise invalidate the cache on every
        // iteration.
        //
        // - Inserted:     dataset schema is exactly `merged_schema`.
        // - Skipped:      insert_source was a no-op, so the schema is
        //                 unchanged → reuse `current_schema`.
        // - Overwritten:  the old layer's exclusive fields may no longer be
        //                 present anywhere, so the schema may shrink in ways
        //                 we can't reconstruct here. Drop the cache; the next
        //                 `schema()` call will pay the full recompute.
        //                 (Overwrite is rare relative to fresh insert in
        //                 registration batches.)
        {
            let mut cache = self.cached_schema.lock();
            let updated_at = self.updated_at();
            *cache = match outcome {
                SourceInsertOutcome::Inserted => Some((updated_at, Arc::new(merged_schema))),
                SourceInsertOutcome::Skipped => Some((updated_at, Arc::new(current_schema))),
                SourceInsertOutcome::Overwritten => None,
            };
        }

        Ok(())
    }

    /// Unregisters segments and layers from the dataset.
    ///
    /// This method acts as a *product* filter:
    /// * `None` `segments_to_drop` + `None` `layers_to_drop`: remove everything
    /// * `None` `segments_to_drop` + `Some` `layers_to_drop`: remove specified layers for *all* segments
    /// * `Some` `segments_to_drop` + `None` `layers_to_drop`: remove *all* layers for specified segments
    /// * `Some` `segments_to_drop` + `Some` `layers_to_drop`: delete *all* specified layers for *all* specified segments
    //
    // we can't expect there are no async calls without the lance feature
    #[allow(clippy::allow_attributes)]
    #[allow(clippy::unused_async)]
    pub async fn remove_layers(
        &mut self,
        segments_to_drop: Option<&HashSet<&SegmentId>>,
        layers_to_drop: Option<&HashSet<&LayerName>>,
    ) -> Result<Vec<(SegmentId, LayerName)>, Error> {
        re_log::debug!(?segments_to_drop, ?layers_to_drop, "remove_layers");

        let mut removed_layers = Vec::new();
        {
            let segments = &mut self.inner.modify().segments;

            // TODO(cmc): we could have fast paths if segments.is_empty() or layers.is_empty() or both.
            segments.retain(|segment_id, segment| {
                if segments_to_drop.is_none_or(|segments| segments.contains(segment_id)) {
                    segment.retain_sources(|layer_name, _source| {
                        if layers_to_drop.is_none_or(|layers| layers.contains(layer_name)) {
                            removed_layers.push((segment_id.clone(), layer_name.clone()));
                            false
                        } else {
                            true
                        }
                    });

                    segment.source_count() > 0
                } else {
                    true
                }
            });
        }

        Ok(removed_layers)
    }

    /// Load a RRD using its recording id as segment id.
    ///
    /// Only stores with matching kinds will be loaded. The stores are registered in the provided
    /// [`StorePool`] automatically.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn register_rrd(
        &mut self,
        pool: &mut StorePool,
        path: &Path,
        layer_name: Option<LayerName>,
        on_duplicate: IfDuplicateBehavior,
        store_kind: StoreKind,
    ) -> Result<BTreeSet<SegmentId>, Error> {
        re_log::info!("Loading {path:?}…");

        let layer_name = layer_name.unwrap_or_else(LayerName::base);
        let layer_info = Arc::new(LayerInfo {
            name: layer_name.clone(),
        });
        let mut new_segment_ids = BTreeSet::default();

        for (store_id, resolved) in ResolvedStore::load_rrd_file(path, store_kind).await? {
            let segment_id = SegmentId::new(store_id.recording_id().to_string());
            let slot_id = pool.register(&resolved);

            self.add_source(
                segment_id.clone(),
                layer_info.clone(),
                slot_id,
                resolved,
                on_duplicate,
            )
            .await?;
            new_segment_ids.insert(segment_id);
        }

        Ok(new_segment_ids)
    }
}
