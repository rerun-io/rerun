use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arrow::array::{BinaryArray, RecordBatch, RecordBatchOptions};
use arrow::datatypes::Schema;
use arrow::error::ArrowError;
use re_byte_size::SizeBytes as _;
use re_log_encoding::RawRrdManifest;
use re_log_types::{AbsoluteTimeRange, Timeline};

use super::StoreSlotId;
use super::resolved_store::ResolvedStore;

#[derive(Clone)]
pub struct Layer {
    store_slot_id: StoreSlotId,
    resolved: ResolvedStore,
    registration_time: jiff::Timestamp,
}

impl Layer {
    pub fn new(store_slot_id: StoreSlotId, resolved: ResolvedStore) -> Self {
        Self {
            store_slot_id,
            resolved,
            registration_time: jiff::Timestamp::now(),
        }
    }

    pub fn store_slot_id(&self) -> StoreSlotId {
        self.store_slot_id
    }

    pub fn resolved_store(&self) -> &ResolvedStore {
        &self.resolved
    }

    pub fn registration_time(&self) -> jiff::Timestamp {
        self.registration_time
    }

    pub fn last_updated_at(&self) -> jiff::Timestamp {
        //TODO(ab): change this if we ever mutate a layer somehow?
        self.registration_time
    }

    #[expect(clippy::unused_self)]
    pub fn layer_type(&self) -> &'static str {
        //TODO(ab): what should that actually be?
        "rrd"
    }

    pub fn num_chunks(&self) -> u64 {
        match &self.resolved {
            ResolvedStore::Eager(h) => h.read().num_physical_chunks() as u64,
            ResolvedStore::Lazy(l) => l.num_chunks() as u64,
        }
    }

    /// Approximate size of this layer.
    ///
    /// The unit differs by backing store and the two values are **not directly comparable**:
    ///
    /// - **Eager** layers report the in-memory heap size of the materialized chunks.
    /// - **Lazy** layers report the on-disk IPC byte length from the RRD footer, including
    ///   each chunk's message header. Chunks are not materialized.
    ///
    /// Treat this as a rough load indicator, not a precise accounting.
    pub fn size_bytes(&self) -> u64 {
        match &self.resolved {
            ResolvedStore::Eager(h) => h
                .read()
                .iter_physical_chunks()
                .map(|chunk| chunk.heap_size_bytes())
                .sum(),

            ResolvedStore::Lazy(l) => {
                let header = re_log_encoding::MessageHeader::ENCODED_SIZE_BYTES as u64;
                l.manifest()
                    .col_chunk_byte_size()
                    .iter()
                    .map(|size| size + header)
                    .sum()
            }
        }
    }

    pub fn schema(&self) -> Schema {
        let fields = self
            .resolved
            .schema()
            .chunk_column_descriptors()
            .arrow_fields();
        Schema::new_with_metadata(fields, HashMap::default())
    }

    pub fn schema_sha256(&self) -> Result<[u8; 32], ArrowError> {
        re_log_encoding::RawRrdManifest::compute_sorbet_schema_sha256(&self.schema())
    }

    pub fn compute_properties(&self) -> Result<RecordBatch, super::Error> {
        self.resolved.extract_properties()
    }

    /// Produce a [`RawRrdManifest`] for this layer, with a `chunk_key` column already populated.
    ///
    /// - **Lazy** layers clone the cached RRD footer manifest — no chunk materialization.
    /// - **Eager** layers rebuild the manifest by iterating every physical chunk.
    ///
    /// The `store_id` on the returned manifest is the layer's own store id; callers merging
    /// multiple layer manifests into a segment-scoped manifest should override it afterwards
    /// (see [`re_log_encoding::RawRrdManifest::merge`]).
    pub fn rrd_manifest(&self) -> Result<RawRrdManifest, super::Error> {
        match &self.resolved {
            ResolvedStore::Lazy(lazy) => self.rrd_manifest_from_lazy_cache(lazy),
            ResolvedStore::Eager(handle) => self.rrd_manifest_from_chunks(handle),
        }
    }

    fn rrd_manifest_from_lazy_cache(
        &self,
        lazy: &Arc<re_chunk_store::LazyRrdStore>,
    ) -> Result<RawRrdManifest, super::Error> {
        let mut manifest = (**lazy.raw_manifest()).clone();

        let chunk_keys = manifest
            .col_chunk_id()
            .map_err(|err| super::Error::RrdLoadingError(err.into()))?
            .map(|chunk_id| {
                super::ChunkKey {
                    chunk_id,
                    store_slot_id: self.store_slot_id,
                }
                .encode()
            })
            .collect::<Result<Vec<_>, _>>()?;

        append_chunk_key_column(&mut manifest, &chunk_keys)?;
        Ok(manifest)
    }

    fn rrd_manifest_from_chunks(
        &self,
        handle: &re_chunk_store::ChunkStoreHandle,
    ) -> Result<RawRrdManifest, super::Error> {
        let store = handle.read();
        let chunks: Vec<Arc<re_chunk_store::Chunk>> =
            store.iter_physical_chunks().cloned().collect();
        let store_id = store.id().clone();
        drop(store);

        let mut builder = re_log_encoding::RrdManifestBuilder::default();
        let mut chunk_keys = Vec::with_capacity(chunks.len());
        let mut offset = 0;

        for chunk in &chunks {
            let chunk_batch = chunk
                .to_chunk_batch()
                .map_err(|err| super::Error::RrdLoadingError(anyhow::anyhow!(err)))?;

            // There's no compression on the OSS server (no disk), so "compressed size" equals
            // uncompressed size. The chunk_key is what's used to actually fetch data.
            let byte_size_uncompressed = chunk.heap_size_bytes();
            let uncompressed_byte_span = re_span::Span {
                start: offset,
                len: byte_size_uncompressed,
            };
            offset += byte_size_uncompressed;

            builder
                .append(&chunk_batch, uncompressed_byte_span, byte_size_uncompressed)
                .map_err(|err| super::Error::RrdLoadingError(err.into()))?;

            chunk_keys.push(
                super::ChunkKey {
                    chunk_id: chunk.id(),
                    store_slot_id: self.store_slot_id,
                }
                .encode()?,
            );
        }

        let mut manifest = builder
            .build(store_id)
            .map_err(|err| super::Error::RrdLoadingError(err.into()))?;

        append_chunk_key_column(&mut manifest, &chunk_keys)?;
        Ok(manifest)
    }

    pub fn index_ranges(&self) -> BTreeMap<Timeline, AbsoluteTimeRange> {
        match &self.resolved {
            ResolvedStore::Eager(h) => {
                let mut ranges = BTreeMap::new();
                for chunk in h.read().iter_physical_chunks() {
                    for time_col in chunk.timelines().values() {
                        let timeline = time_col.timeline().to_owned();
                        let range = time_col.time_range();
                        let entry = ranges.entry(timeline).or_insert(range);
                        *entry = entry.union(range);
                    }
                }
                ranges
            }
            ResolvedStore::Lazy(l) => {
                let mut ranges = BTreeMap::new();
                for per_entity in l.manifest().temporal_map().values() {
                    for (timeline, per_component) in per_entity {
                        for per_chunk in per_component.values() {
                            for entry in per_chunk.values() {
                                let range = entry.time_range;
                                let e = ranges.entry(*timeline).or_insert(range);
                                *e = e.union(range);
                            }
                        }
                    }
                }
                ranges
            }
        }
    }
}

/// Append the server-synthesized `chunk_key` column to a [`RawRrdManifest`].
///
/// The keys must be aligned with `manifest.data`'s existing rows.
fn append_chunk_key_column(
    manifest: &mut RawRrdManifest,
    chunk_keys: &[Vec<u8>],
) -> Result<(), super::Error> {
    let (schema, mut columns, num_rows) = manifest.data.clone().into_parts();

    let schema = {
        let mut schema = Arc::unwrap_or_clone(schema);
        let mut fields = schema.fields.to_vec();
        fields.push(Arc::new(RawRrdManifest::field_chunk_key()));
        schema.fields = fields.into();
        schema
    };

    let keys_array = BinaryArray::from_iter_values(chunk_keys.iter());
    columns.push(Arc::new(keys_array));

    manifest.data = RecordBatch::try_new_with_options(
        Arc::new(schema),
        columns,
        &RecordBatchOptions::new().with_row_count(Some(num_rows)),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use arrow::array::Array as _;
    use re_arrow_util::ArrowArrayDowncastRef as _;
    use re_chunk_store::external::re_chunk;
    use re_chunk_store::{Chunk, ChunkStore, ChunkStoreConfig, ChunkStoreHandle, LazyRrdStore};
    use re_log_encoding::EncodingOptions;
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, TimePoint,
        Timeline,
        example_components::{MyPoint, MyPoints},
    };
    use re_types_core::ChunkId;

    use super::*;
    use crate::store::{ChunkKey, ResolvedStore};

    fn build_chunks() -> (StoreId, Vec<Arc<Chunk>>) {
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let timeline = Timeline::new_sequence("frame");
        let mut chunks = Vec::new();
        for entity_idx in 0..2 {
            for frame_idx in 0..3i64 {
                let entity_path = EntityPath::from(format!("/entity_{entity_idx}"));
                let points = MyPoint::from_iter(
                    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        frame_idx as u32..frame_idx as u32 + 1
                    },
                );
                let chunk = Chunk::builder(entity_path)
                    .with_sparse_component_batches(
                        re_chunk::RowId::new(),
                        TimePoint::default().with(timeline, frame_idx),
                        [(MyPoints::descriptor_points(), Some(&points as _))],
                    )
                    .build()
                    .unwrap();
                chunks.push(Arc::new(chunk));
            }
        }
        (store_id, chunks)
    }

    fn write_rrd(path: &Path, store_id: &StoreId, chunks: &[Arc<Chunk>]) {
        let set_store_info = LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *re_chunk::RowId::ZERO,
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        });
        let mut file = std::fs::File::create(path).unwrap();
        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_log_encoding::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .unwrap();
        encoder.append(&set_store_info).unwrap();
        for chunk in chunks {
            let arrow_msg = chunk.to_arrow_msg().unwrap();
            let msg = LogMsg::ArrowMsg(store_id.clone(), arrow_msg);
            encoder.append(&msg).unwrap();
        }
        encoder.finish().unwrap();
    }

    /// Single-layer equivalence: a Lazy-backed layer and an Eager-backed layer holding the same
    /// chunks must produce manifests that are equivalent on the axes clients care about (chunk
    /// IDs, entity paths, staticness, row counts, schema shape, decodable `chunk_key`s).
    ///
    /// Byte-size/offset columns are intentionally NOT compared: per the `RawRrdManifest`
    /// docstring, Lazy reports on-disk IPC sizes while Eager reports heap sizes.
    #[test]
    fn rrd_manifest_lazy_and_eager_produce_equivalent_output() {
        let (store_id, chunks) = build_chunks();

        // Eager backend: in-memory `ChunkStore`. `ALL_DISABLED` matches `LazyRrdStore`'s internal
        // config, so both sides hold the same chunk set (otherwise compaction on insert would
        // merge them and the manifests would no longer be row-wise comparable).
        let mut eager_store = ChunkStore::new(store_id.clone(), ChunkStoreConfig::ALL_DISABLED);
        for chunk in &chunks {
            eager_store.insert_chunk(chunk).unwrap();
        }
        let eager_layer = Layer::new(
            StoreSlotId::new(),
            ResolvedStore::Eager(ChunkStoreHandle::new(eager_store)),
        );

        // Lazy backend: same chunks, written to an RRD file with footer, then loaded lazily.
        let dir = tempfile::tempdir().unwrap();
        let rrd_path = dir.path().join("test.rrd");
        write_rrd(&rrd_path, &store_id, &chunks);

        let mut footer_file = std::fs::File::open(&rrd_path).unwrap();
        let footer = re_log_encoding::read_rrd_footer(&mut footer_file)
            .unwrap()
            .unwrap();
        let raw_manifest = Arc::new(footer.manifests[&store_id].clone());
        let store_file = std::fs::File::open(&rrd_path).unwrap();
        let lazy =
            Arc::new(LazyRrdStore::try_new(store_file, rrd_path.clone(), raw_manifest).unwrap());
        let lazy_layer = Layer::new(StoreSlotId::new(), ResolvedStore::Lazy(lazy));

        let lazy_manifest = lazy_layer.rrd_manifest().unwrap();
        let eager_manifest = eager_layer.rrd_manifest().unwrap();

        // Row counts match.
        assert_eq!(
            lazy_manifest.data.num_rows(),
            eager_manifest.data.num_rows(),
            "row counts differ"
        );

        // Chunk IDs match as sets (per-row order is not part of the contract).
        let lazy_ids: BTreeSet<ChunkId> = lazy_manifest.col_chunk_id().unwrap().collect();
        let eager_ids: BTreeSet<ChunkId> = eager_manifest.col_chunk_id().unwrap().collect();
        assert_eq!(lazy_ids, eager_ids, "chunk IDs differ");

        // Compare per-chunk metadata. Both manifests may list chunks in different orders, so
        // sort by chunk_id first.
        let sort_by_chunk_id = |manifest: &RawRrdManifest| -> Vec<usize> {
            let mut indexed: Vec<(usize, ChunkId)> =
                manifest.col_chunk_id().unwrap().enumerate().collect();
            indexed.sort_by_key(|(_, id)| *id);
            indexed.into_iter().map(|(i, _)| i).collect()
        };
        let lazy_order = sort_by_chunk_id(&lazy_manifest);
        let eager_order = sort_by_chunk_id(&eager_manifest);

        let lazy_entity_paths = lazy_manifest.col_chunk_entity_path_raw().unwrap();
        let eager_entity_paths = eager_manifest.col_chunk_entity_path_raw().unwrap();
        let lazy_is_static = lazy_manifest.col_chunk_is_static_raw().unwrap();
        let eager_is_static = eager_manifest.col_chunk_is_static_raw().unwrap();
        let lazy_num_rows = lazy_manifest.col_chunk_num_rows_raw().unwrap();
        let eager_num_rows = eager_manifest.col_chunk_num_rows_raw().unwrap();

        for (li, ei) in lazy_order.iter().zip(eager_order.iter()) {
            assert_eq!(
                lazy_entity_paths.value(*li),
                eager_entity_paths.value(*ei),
                "entity_path differs"
            );
            assert_eq!(
                lazy_is_static.value(*li),
                eager_is_static.value(*ei),
                "is_static differs"
            );
            assert_eq!(
                lazy_num_rows.value(*li),
                eager_num_rows.value(*ei),
                "num_rows differs"
            );
        }

        // Sorbet schema SHA matches: both paths describe the same logical recording schema.
        assert_eq!(
            lazy_manifest.sorbet_schema_sha256, eager_manifest.sorbet_schema_sha256,
            "sorbet schema SHA differs between lazy and eager"
        );

        // Manifest RecordBatch schemas match too: same columns (chunk_fetcher base columns,
        // dynamically-emitted per-timeline/component index columns, plus the appended
        // `chunk_key`). If either code path forgets a column or reorders fields, we'd diverge.
        assert_eq!(
            lazy_manifest.data.schema(),
            eager_manifest.data.schema(),
            "manifest RecordBatch schema differs between lazy and eager"
        );

        // `chunk_key` column is present on both and decodes to in-manifest chunk IDs.
        let decode_keys = |manifest: &RawRrdManifest| -> BTreeSet<ChunkId> {
            let keys: &BinaryArray = manifest
                .data
                .column_by_name(RawRrdManifest::FIELD_CHUNK_KEY)
                .expect("chunk_key column missing")
                .downcast_array_ref::<BinaryArray>()
                .unwrap();
            (0..keys.len())
                .map(|i| ChunkKey::decode(keys.value(i)).unwrap().chunk_id)
                .collect()
        };
        assert_eq!(decode_keys(&lazy_manifest), lazy_ids);
        assert_eq!(decode_keys(&eager_manifest), eager_ids);
    }
}
