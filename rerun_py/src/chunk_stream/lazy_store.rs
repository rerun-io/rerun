use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use pyo3::prelude::*;

use re_chunk::{Chunk, ChunkId};
use re_chunk_store::LazyStore;
use re_log_encoding::{RrdManifest, RrdManifestStaticMap, RrdManifestTemporalMap};
use re_log_types::EntityPath;
use re_types_core::{ComponentIdentifier, TimelineName};

use super::engine::FilterStream;
use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::{ChunkPredicateView, LazyChunkStream, StructuredFilter};
use super::summary::{SummaryRow, format_summary};
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::PySchemaInternal;

/// An index-based, lazily-loaded chunk store.
///
/// Constructed from a [`LazyStore`]; the manifest is held in memory but chunks are loaded on
/// demand. Implements [`ChunkStreamFactory`] so `stream()` produces an [`IndexedChunkStream`]
/// that pulls chunks in byte-budgeted batches.
#[pyclass(
    frozen,
    name = "LazyStoreInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyLazyStoreInternal {
    inner: Arc<LazyStore>,
}

impl PyLazyStoreInternal {
    pub fn new(lazy: LazyStore) -> Self {
        Self {
            inner: Arc::new(lazy),
        }
    }
}

#[pymethods]
impl PyLazyStoreInternal {
    /// The schema describing all columns in this store.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self.inner.schema().chunk_column_descriptors().into(),
            metadata: Default::default(),
        }
    }

    /// The total number of chunks described by the manifest (virtual and physical).
    fn num_chunks(&self) -> usize {
        self.inner.manifest().num_chunks()
    }

    /// Monotonic count of chunks physically loaded from this store since it was opened.
    ///
    /// Exposed as `_chunks_loaded` (underscore-prefixed) — intended for test-side validation
    /// that pushdown / lazy loading is engaged. Not a performance metric and not part of the
    /// stable public API.
    #[getter(_chunks_loaded)]
    fn chunks_loaded(&self) -> u64 {
        self.inner.chunks_loaded()
    }

    /// Compact, deterministic summary of every chunk in the store for snapshot testing.
    ///
    /// Each line describes one chunk:
    /// `{entity_path} rows={n} static={bool} timelines=[…] cols=[…]`
    ///
    /// Built from the manifest only — no chunk data is loaded.
    fn summary(&self) -> String {
        let manifest = self.inner.manifest();
        let chunk_ids = manifest.col_chunk_ids();
        let entity_paths = manifest.col_chunk_entity_path_raw();
        let is_static_iter: Vec<bool> = manifest.col_chunk_is_static().collect();
        let num_rows = manifest.col_chunk_num_rows();

        // Per-chunk (timelines, cols), using BTreeSet for sorted-by-construction order.
        let mut per_chunk: HashMap<ChunkId, (BTreeSet<&'static str>, BTreeSet<&'static str>)> =
            HashMap::new();

        for per_entity in manifest.temporal_map().values() {
            for (timeline, per_component) in per_entity {
                let timeline_name = timeline.name().as_str();
                for (component, per_chunk_map) in per_component {
                    let component_name = component.as_str();
                    for chunk_id in per_chunk_map.keys() {
                        let entry = per_chunk.entry(*chunk_id).or_default();
                        entry.0.insert(timeline_name);
                        entry.1.insert(timeline_name);
                        entry.1.insert(component_name);
                    }
                }
            }
        }
        for per_entity in manifest.static_map().values() {
            for (component, chunk_id) in per_entity {
                let entry = per_chunk.entry(*chunk_id).or_default();
                entry.1.insert(component.as_str());
            }
        }

        let rows = chunk_ids.iter().enumerate().map(|(i, id)| {
            let (timelines, cols) = per_chunk.remove(id).unwrap_or_default();
            SummaryRow {
                entity_path: entity_paths.value(i).to_owned(),
                num_rows: num_rows[i],
                is_static: is_static_iter[i],
                timelines: timelines.into_iter().map(str::to_owned).collect(),
                cols: cols.into_iter().map(str::to_owned).collect(),
            }
        });
        format_summary(rows)
    }

    /// Return a lazy stream over all chunks in this store.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(Arc::clone(&self.inner)))
    }
}

/// `Arc<LazyStore>` is itself the factory: it owns the manifest and serves on-demand chunk
/// loads, which is exactly what a [`ChunkStreamFactory`] needs.
impl ChunkStreamFactory for Arc<LazyStore> {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        Ok(Box::new(IndexedChunkStream::new(Self::clone(self))))
    }

    fn create_with_pushdown(
        &self,
        filter: &StructuredFilter,
    ) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let manifest = self.manifest();
        let (matching_ids, remainder) = evaluate_filter_on_manifest(filter, manifest);

        let stream: Box<dyn ChunkStream> = Box::new(IndexedChunkStream::new_with_ids(
            Self::clone(self),
            matching_ids,
        ));
        Ok(match remainder {
            Some(rem) => Box::new(FilterStream::new(stream, rem)),
            None => stream,
        })
    }
}

/// Evaluate chunk-level predicates against the manifest.
///
/// Returns `(matching_chunk_ids, remainder_filter)`. The remainder is non-`None` only when
/// component column slicing must still run post-load — chunks that pass the predicate may
/// still carry columns we want to drop.
fn evaluate_filter_on_manifest(
    filter: &StructuredFilter,
    manifest: &RrdManifest,
) -> (Vec<ChunkId>, Option<StructuredFilter>) {
    let chunk_ids = manifest.col_chunk_ids();

    //TODO(perf): `col_chunk_entity_path()` parses+interns one `EntityPath` per chunk.
    // When `filter.content.is_none()`, we don't need the parsed form at all, and could
    // iterate `col_chunk_entity_path_raw()` (a `&StringArray`) instead, parsing only when
    // a temporal/static_map lookup actually requires it. Skipped for v1; revisit when
    // profiling points here.
    let entity_paths: Vec<EntityPath> = manifest.col_chunk_entity_path().collect();
    let is_static_col: Vec<bool> = manifest.col_chunk_is_static().collect();

    let temporal_map = manifest.temporal_map();
    let static_map = manifest.static_map();

    let matching: Vec<ChunkId> = chunk_ids
        .iter()
        .zip(&entity_paths)
        .zip(&is_static_col)
        .filter_map(|((&chunk_id, entity_path), &is_static)| {
            let view = ManifestRow {
                chunk_id,
                entity_path,
                is_static,
                temporal_map,
                static_map,
            };
            filter.matches(&view).then_some(chunk_id)
        })
        .collect();

    // Remainder: `components` slices columns post-load. The predicate above already drops
    // chunks that have *none* of the listed components, but each surviving chunk may still
    // carry unwanted columns that the `FilterStream` will trim.
    let remainder = filter.components.as_ref().map(|c| StructuredFilter {
        content: None,
        has_timeline: None,
        is_static: None,
        components: Some(c.clone()),
    });

    (matching, remainder)
}

/// Per-row view into an [`RrdManifest`] for [`StructuredFilter::matches`].
///
/// Holds references into the manifest's chunk-id, entity-path, static/temporal maps so the
/// trait methods can answer `has_timeline` / `has_any_component` with the same lookups the
/// previous inline loop did — no allocations, scoped to one row of the manifest.
struct ManifestRow<'a> {
    chunk_id: ChunkId,
    entity_path: &'a EntityPath,
    is_static: bool,
    temporal_map: &'a RrdManifestTemporalMap,
    static_map: &'a RrdManifestStaticMap,
}

impl ChunkPredicateView for ManifestRow<'_> {
    fn entity_path(&self) -> &EntityPath {
        self.entity_path
    }

    fn is_static(&self) -> bool {
        self.is_static
    }

    fn has_timeline(&self, name: &TimelineName) -> bool {
        if self.is_static {
            // Static chunks have no timelines.
            return false;
        }
        self.temporal_map
            .get(self.entity_path)
            .and_then(|per_tl| per_tl.iter().find(|(tl, _)| tl.name() == name))
            .is_some_and(|(_, per_comp)| {
                per_comp
                    .values()
                    .any(|per_chunk| per_chunk.contains_key(&self.chunk_id))
            })
    }

    fn has_any_component(&self, components: &[ComponentIdentifier]) -> bool {
        if self.is_static {
            self.static_map
                .get(self.entity_path)
                .is_some_and(|per_comp| {
                    components
                        .iter()
                        .any(|c| per_comp.get(c).is_some_and(|id| *id == self.chunk_id))
                })
        } else {
            self.temporal_map
                .get(self.entity_path)
                .is_some_and(|per_tl| {
                    per_tl.values().any(|per_comp| {
                        components.iter().any(|c| {
                            per_comp
                                .get(c)
                                .is_some_and(|per_chunk| per_chunk.contains_key(&self.chunk_id))
                        })
                    })
                })
        }
    }
}

// --- Streaming ---

/// Streaming loader for an indexed (lazy) [`ChunkStore`].
///
/// Pulls chunks from the underlying [`ChunkProvider`][re_log_encoding::ChunkProvider] in
/// byte-budgeted batches so resident memory stays bounded regardless of total recording size.
//TODO(RR-4545): this is hardly an optimal strategy. We need the ChunkProvider to expose a streaming
// API so that specific optimizations can be applied (e.g. adjacency for RRD, parallelism for
// segments, etc.)
struct IndexedChunkStream {
    lazy: Arc<LazyStore>,
    chunk_ids: Vec<ChunkId>,
    next_id: usize,
    buffer: VecDeque<Arc<Chunk>>,
}

impl IndexedChunkStream {
    /// Target bytes per batch — bounds memory while still letting `read_chunks` coalesce.
    const BATCH_BYTE_BUDGET: u64 = 8 * 1024 * 1024;

    /// Stream all chunks in the manifest (current behavior).
    fn new(lazy: Arc<LazyStore>) -> Self {
        let chunk_ids = lazy.manifest().col_chunk_ids().to_vec();
        Self::new_with_ids(lazy, chunk_ids)
    }

    /// Stream only the given chunk IDs (used by pushdown).
    ///
    /// IDs that do not appear in the manifest are tolerated — `next_batch_end` assigns
    /// them a size of `0` via [`LazyStore::chunk_row_index`]'s `None` branch, and
    /// `load_chunks` is the layer that would ultimately reject them. Manifest membership
    /// is the caller's invariant.
    fn new_with_ids(lazy: Arc<LazyStore>, chunk_ids: Vec<ChunkId>) -> Self {
        Self {
            lazy,
            chunk_ids,
            next_id: 0,
            buffer: VecDeque::new(),
        }
    }

    /// End index (exclusive) of the next batch starting at `self.next_id`,
    /// chosen so the cumulative byte size stays under [`Self::BATCH_BYTE_BUDGET`].
    /// Always advances by at least one chunk to guarantee progress on huge chunks.
    fn next_batch_end(&self) -> usize {
        let sizes = self.lazy.manifest().col_chunk_byte_size();
        let mut end = self.next_id;
        let mut accumulated: u64 = 0;
        while end < self.chunk_ids.len() {
            let size = self
                .lazy
                .chunk_row_index(&self.chunk_ids[end])
                .map(|row| sizes[row])
                .unwrap_or(0);
            if end > self.next_id && accumulated.saturating_add(size) > Self::BATCH_BYTE_BUDGET {
                break;
            }
            accumulated = accumulated.saturating_add(size);
            end += 1;
        }
        end
    }
}

impl ChunkStream for IndexedChunkStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        loop {
            if let Some(chunk) = self.buffer.pop_front() {
                return Ok(Some(chunk));
            }
            if self.next_id >= self.chunk_ids.len() {
                return Ok(None);
            }

            let end = self.next_batch_end();
            let ids = &self.chunk_ids[self.next_id..end];
            let chunks =
                self.lazy
                    .load_chunks(ids)
                    .map_err(|err| ChunkPipelineError::IndexedLoad {
                        from: self.lazy.source(),
                        reason: err.to_string(),
                    })?;
            self.next_id = end;
            self.buffer = chunks.into();
        }
    }
}

#[cfg(test)]
mod pushdown_tests {
    //! Unit tests for [`evaluate_filter_on_manifest`].

    use std::fs::File;
    use std::path::Path;

    use re_chunk::{Chunk, RowId, TimePoint, Timeline};
    use re_log_encoding::{EncodingOptions, RrdChunkProvider};
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
    };
    use re_types_core::{ComponentDescriptor, ComponentIdentifier, TimelineName};

    use super::*;

    /// Which example component a test chunk should carry.
    #[derive(Copy, Clone)]
    pub(crate) enum TestComponent {
        Points,
        Colors,
    }

    impl TestComponent {
        fn descriptor(self) -> ComponentDescriptor {
            match self {
                Self::Points => MyPoints::descriptor_points(),
                Self::Colors => MyPoints::descriptor_colors(),
            }
        }

        pub(crate) fn identifier(self) -> ComponentIdentifier {
            self.descriptor().component
        }
    }

    /// Per-chunk recipe.
    pub(crate) struct ChunkSpec<'a> {
        pub entity: &'a str,
        pub component: TestComponent,
        pub is_static: bool,
        pub num_frames: usize,
    }

    pub(crate) fn build_test_store(specs: &[ChunkSpec<'_>]) -> (Arc<LazyStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rrd");
        let store = build_test_store_at(&path, specs);
        (store, dir)
    }

    fn build_test_store_at(path: &Path, specs: &[ChunkSpec<'_>]) -> Arc<LazyStore> {
        let store_id = StoreId::random(StoreKind::Recording, "pushdown-test");
        let timeline = Timeline::new_sequence("frame");

        let mut chunks: Vec<Arc<Chunk>> = Vec::new();
        for spec in specs {
            let entity = EntityPath::from(spec.entity);
            let descriptor = spec.component.descriptor();

            if spec.is_static {
                let row_id = RowId::new();
                let chunk = match spec.component {
                    TestComponent::Points => {
                        let points = MyPoint::from_iter(0..1);
                        Chunk::builder(entity)
                            .with_sparse_component_batches(
                                row_id,
                                TimePoint::default(),
                                [(descriptor, Some(&points as _))],
                            )
                            .build()
                            .unwrap()
                    }
                    TestComponent::Colors => {
                        let colors = MyColor::from_iter([0xFF_00_00_FFu32]);
                        Chunk::builder(entity)
                            .with_sparse_component_batches(
                                row_id,
                                TimePoint::default(),
                                [(descriptor, Some(&colors as _))],
                            )
                            .build()
                            .unwrap()
                    }
                };
                chunks.push(Arc::new(chunk));
            } else {
                let mut builder = Chunk::builder(entity);
                for frame in 0..spec.num_frames {
                    let row_id = RowId::new();
                    #[expect(clippy::cast_possible_wrap)]
                    let timepoint = TimePoint::default().with(timeline, frame as i64);
                    builder = match spec.component {
                        TestComponent::Points => {
                            #[expect(clippy::cast_possible_truncation)]
                            let points = MyPoint::from_iter(frame as u32..frame as u32 + 1);
                            builder.with_sparse_component_batches(
                                row_id,
                                timepoint,
                                [(descriptor.clone(), Some(&points as _))],
                            )
                        }
                        TestComponent::Colors => {
                            #[expect(clippy::cast_possible_truncation)]
                            let colors = MyColor::from_iter([frame as u32 + 1]);
                            builder.with_sparse_component_batches(
                                row_id,
                                timepoint,
                                [(descriptor.clone(), Some(&colors as _))],
                            )
                        }
                    };
                }
                chunks.push(Arc::new(builder.build().unwrap()));
            }
        }

        let mut file = std::fs::File::create(path).unwrap();
        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_log_encoding::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .unwrap();
        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
            }))
            .unwrap();
        for chunk in &chunks {
            let arrow_msg = chunk.to_arrow_msg().unwrap();
            encoder
                .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
                .unwrap();
        }
        encoder.finish().unwrap();

        let mut file = File::open(path).unwrap();
        let footer = re_log_encoding::read_rrd_footer(&mut file)
            .unwrap()
            .unwrap();
        let raw = Arc::new(footer.manifests[&store_id].clone());
        let file = File::open(path).unwrap();
        let provider = Arc::new(RrdChunkProvider::try_from_file(file, path, raw).unwrap());
        Arc::new(LazyStore::new(provider))
    }

    fn epf(rules: &str) -> re_log_types::ResolvedEntityPathFilter {
        re_log_types::EntityPathFilter::parse_forgiving(rules).resolve_without_substitutions()
    }

    fn ids_for_entity(store: &LazyStore, entity: &str) -> Vec<ChunkId> {
        let path = EntityPath::from(entity);
        let manifest = store.manifest();
        let entity_paths: Vec<EntityPath> = manifest.col_chunk_entity_path().collect();
        manifest
            .col_chunk_ids()
            .iter()
            .zip(&entity_paths)
            .filter_map(|(id, p)| if p == &path { Some(*id) } else { None })
            .collect()
    }

    fn sort_ids(mut v: Vec<ChunkId>) -> Vec<ChunkId> {
        v.sort();
        v
    }

    #[test]
    fn test_eval_entity_path() {
        let (store, _dir) = build_test_store(&[
            ChunkSpec {
                entity: "/robot",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 2,
            },
            ChunkSpec {
                entity: "/camera",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 2,
            },
        ]);

        let filter = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            ..Default::default()
        };
        let (matching, remainder) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert!(remainder.is_none());
        assert_eq!(
            sort_ids(matching),
            sort_ids(ids_for_entity(&store, "/robot"))
        );
    }

    #[test]
    fn test_eval_is_static() {
        let (store, _dir) = build_test_store(&[
            ChunkSpec {
                entity: "/static_one",
                component: TestComponent::Points,
                is_static: true,
                num_frames: 0,
            },
            ChunkSpec {
                entity: "/temporal",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 2,
            },
        ]);

        let filter = StructuredFilter {
            is_static: Some(true),
            ..Default::default()
        };
        let (matching, remainder) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert!(remainder.is_none());
        assert_eq!(
            sort_ids(matching),
            sort_ids(ids_for_entity(&store, "/static_one"))
        );
    }

    #[test]
    fn test_eval_has_timeline() {
        let (store, _dir) = build_test_store(&[
            ChunkSpec {
                entity: "/temporal",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 2,
            },
            ChunkSpec {
                entity: "/static_one",
                component: TestComponent::Points,
                is_static: true,
                num_frames: 0,
            },
        ]);

        let filter = StructuredFilter {
            has_timeline: Some(TimelineName::new("frame")),
            ..Default::default()
        };
        let (matching, _) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert_eq!(
            sort_ids(matching),
            sort_ids(ids_for_entity(&store, "/temporal"))
        );

        // A non-existent timeline should match nothing.
        let filter = StructuredFilter {
            has_timeline: Some(TimelineName::new("never_logged")),
            ..Default::default()
        };
        let (matching, _) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert!(matching.is_empty());
    }

    #[test]
    fn test_eval_components() {
        let (store, _dir) = build_test_store(&[
            ChunkSpec {
                entity: "/a",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 1,
            },
            ChunkSpec {
                entity: "/b",
                component: TestComponent::Colors,
                is_static: false,
                num_frames: 1,
            },
        ]);

        let filter = StructuredFilter {
            components: Some(vec![TestComponent::Points.identifier()]),
            ..Default::default()
        };
        let (matching, remainder) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert_eq!(sort_ids(matching), sort_ids(ids_for_entity(&store, "/a")));

        let remainder = remainder.expect("components remainder must be present for slicing");
        assert!(remainder.content.is_none());
        assert!(remainder.has_timeline.is_none());
        assert!(remainder.is_static.is_none());
        assert_eq!(
            remainder.components,
            Some(vec![TestComponent::Points.identifier()])
        );
    }

    #[test]
    fn test_eval_combined() {
        let (store, _dir) = build_test_store(&[
            ChunkSpec {
                entity: "/robot",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 1,
            },
            ChunkSpec {
                entity: "/robot",
                component: TestComponent::Points,
                is_static: true,
                num_frames: 0,
            },
            ChunkSpec {
                entity: "/camera",
                component: TestComponent::Points,
                is_static: false,
                num_frames: 1,
            },
        ]);

        let filter = StructuredFilter {
            content: Some(epf("+ /robot/**")),
            is_static: Some(false),
            ..Default::default()
        };
        let (matching, _) = evaluate_filter_on_manifest(&filter, store.manifest());

        let manifest = store.manifest();
        let is_static_col: Vec<bool> = manifest.col_chunk_is_static().collect();
        let entity_paths: Vec<EntityPath> = manifest.col_chunk_entity_path().collect();
        let expected: Vec<ChunkId> = manifest
            .col_chunk_ids()
            .iter()
            .zip(&entity_paths)
            .zip(&is_static_col)
            .filter_map(|((id, ep), &is_static)| {
                (ep == &EntityPath::from("/robot") && !is_static).then_some(*id)
            })
            .collect();
        assert_eq!(sort_ids(matching), sort_ids(expected));
    }

    #[test]
    fn test_eval_no_match() {
        let (store, _dir) = build_test_store(&[ChunkSpec {
            entity: "/robot",
            component: TestComponent::Points,
            is_static: false,
            num_frames: 1,
        }]);

        let filter = StructuredFilter {
            content: Some(epf("+ /nope/**")),
            ..Default::default()
        };
        let (matching, _) = evaluate_filter_on_manifest(&filter, store.manifest());
        assert!(matching.is_empty());
    }
}
