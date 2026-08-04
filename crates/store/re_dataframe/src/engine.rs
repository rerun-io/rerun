use re_chunk::EntityPath;
// Used all over in docstrings.
#[expect(unused_imports)]
use re_chunk_store::ComponentColumnDescriptor;
use re_chunk_store::{ChunkStoreHandle, QueryExpression};
use re_log_types::EntityPathFilter;
use re_query::{QueryCache, QueryCacheHandle, StorageEngine, StorageEngineLike};
use re_sorbet::{ChunkColumnDescriptors, ColumnDescriptor};

use crate::QueryHandle;
use crate::query::compute_user_selection;

// --- Queries ---

/// A handle to our user-facing query engine.
///
/// Cheap to clone.
///
/// See the following methods:
/// * [`QueryEngine::schema`]: get the complete schema of the recording.
/// * [`QueryEngine::query`]: execute a [`QueryExpression`] on the recording.
#[derive(Clone)]
pub struct QueryEngine<E: StorageEngineLike> {
    pub engine: E,
}

impl QueryEngine<StorageEngine> {
    #[inline]
    pub fn new(store: ChunkStoreHandle, cache: QueryCacheHandle) -> Self {
        // Safety: EntityDb's handles can never be accessed from the outside, therefore these
        // handles had to have been constructed in an external context, outside of the main app.
        #[expect(unsafe_code)]
        let engine = unsafe { StorageEngine::new(store, cache) };

        Self { engine }
    }

    /// This will automatically instantiate a new empty [`QueryCache`].
    #[inline]
    pub fn from_store(store: ChunkStoreHandle) -> Self {
        Self::new(store.clone(), QueryCache::new_handle(store))
    }

    /// Loads an RRD file and instantiates [`QueryEngine`]s with new empty [`QueryCache`]s.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_rrd_filepath(
        store_config: &re_chunk_store::ChunkStoreConfig,
        path_to_rrd: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<std::collections::BTreeMap<re_log_types::StoreId, Self>> {
        use anyhow::Context as _;

        let path_to_rrd = path_to_rrd.as_ref();
        re_tracing::profile_function!(path_to_rrd.to_string_lossy());

        let rrd_file = std::fs::File::open(path_to_rrd)
            .with_context(|| format!("couldn't open RRD file\nFile path: {path_to_rrd:?}"))?;

        Ok(
            re_chunk_store::ChunkStore::handle_from_rrd_reader(store_config, rrd_file)
                .with_context(|| format!("couldn't decode RRD file\nFile path: {path_to_rrd:?}"))?
                .into_iter()
                .map(|(store_id, store)| (store_id, Self::from_store(store)))
                .collect(),
        )
    }
}

impl<E: StorageEngineLike + Clone> QueryEngine<E> {
    /// Returns the full schema of the store.
    ///
    /// This will include a column descriptor for every timeline and every component on every
    /// entity that has been written to the store so far.
    ///
    /// The order of the columns to guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema(&self) -> ChunkColumnDescriptors {
        self.engine
            .with(|store, _cache| store.schema().chunk_column_descriptors())
    }

    /// Returns the filtered schema for the given [`QueryExpression`].
    ///
    /// The order of the columns is guaranteed to be in a specific order:
    /// * first, the time columns in lexical order (`frame_nr`, `log_time`, ...);
    /// * second, the component columns in lexical order (`Color`, `Radius, ...`).
    #[inline]
    pub fn schema_for_query(&self, query: &QueryExpression) -> ChunkColumnDescriptors {
        self.engine
            .with(|store, _cache| store.schema_for_query(query))
    }

    /// Returns the column descriptors that will appear in the query's output,
    /// in the order they will appear, after both view-contents filtering and
    /// the user's `selection` (if any) have been applied.
    ///
    /// When `query.selection` is `None`, this is equivalent to
    /// `schema_for_query(query).indices_and_components()` (i.e. all view
    /// columns except `row_id`).
    ///
    /// When `query.selection` is `Some`, the output matches the resolution
    /// performed by [`QueryHandle`] at init time — including synthesizing
    /// placeholder descriptors for selectors that did not hit any column in
    /// the view (those columns will emit all-null values at query time).
    ///
    /// Computed cheaply: no `QueryHandle` is built, no chunks are fetched.
    pub fn selected_schema_for_query(&self, query: &QueryExpression) -> Vec<ColumnDescriptor> {
        let view_contents = self.schema_for_query(query).indices_and_components();
        match query.selection.as_deref() {
            None => view_contents,
            Some(selection) => compute_user_selection(&view_contents, selection)
                .into_iter()
                .map(|(_, descr)| descr)
                .collect(),
        }
    }

    /// Starts a new query by instantiating a [`QueryHandle`].
    #[inline]
    pub fn query(&self, query: QueryExpression) -> QueryHandle<E> {
        QueryHandle::new(self.engine.clone(), query)
    }

    /// Returns an iterator over all the [`EntityPath`]s present in the database.
    #[inline]
    pub fn iter_entity_paths_sorted<'a>(
        &self,
        filter: &'a EntityPathFilter,
    ) -> impl Iterator<Item = EntityPath> + 'a + use<'a, E> {
        let filter = filter.clone().resolve_without_substitutions();
        self.engine.with(|store, _cache| {
            store
                .all_entities_sorted()
                .into_iter()
                .filter(move |entity_path| filter.matches(entity_path))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, TimeInt};
    use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle, QueryExpression};
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPath, EntityPathFilter, TimePoint, build_frame_nr};
    use re_sorbet::ColumnDescriptor;

    use crate::QueryEngine;

    /// Store with two entities, each with a distinct, non-overlapping component.
    fn build_two_entity_store() -> anyhow::Result<ChunkStore> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let points = MyPoint::from_iter(0..1);
        let chunk_a = Chunk::builder(EntityPath::from("/a"))
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(0))],
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()?;
        store.insert_chunk(&Arc::new(chunk_a))?;

        let colors = MyColor::from_iter(0..1);
        let chunk_b = Chunk::builder(EntityPath::from("/b"))
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(0))],
                [(MyPoints::descriptor_colors(), Some(&colors as _))],
            )
            .build()?;
        store.insert_chunk(&Arc::new(chunk_b))?;

        Ok(store)
    }

    /// Store with one color component per entity path, inserted in the given (deliberately
    /// unsorted) order.
    fn build_store_with_entities<'a>(
        entity_paths: impl IntoIterator<Item = &'a str>,
    ) -> anyhow::Result<ChunkStore> {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        for entity_path in entity_paths {
            let colors = MyColor::from_iter(0..1);
            let chunk = Chunk::builder(EntityPath::from(entity_path))
                .with_sparse_component_batches(
                    RowId::new(),
                    TimePoint::default(),
                    [(MyPoints::descriptor_colors(), Some(&colors as _))],
                )
                .build()?;
            store.insert_chunk(&Arc::new(chunk))?;
        }

        Ok(store)
    }

    #[test]
    fn schema_returns_union_of_all_entities() -> anyhow::Result<()> {
        let store = ChunkStoreHandle::new(build_two_entity_store()?);
        let engine = QueryEngine::from_store(store);

        let schema = engine.schema();
        let entity_paths: BTreeSet<_> = schema
            .components
            .iter()
            .map(|c| c.entity_path.clone())
            .collect();

        assert_eq!(
            entity_paths,
            [EntityPath::from("/a"), EntityPath::from("/b")].into()
        );

        Ok(())
    }

    #[test]
    fn schema_for_query_filters_by_view_contents() -> anyhow::Result<()> {
        let store = ChunkStoreHandle::new(build_two_entity_store()?);
        let engine = QueryEngine::from_store(store);

        let query = QueryExpression {
            view_contents: Some(std::iter::once((EntityPath::from("/a"), None)).collect()),
            ..Default::default()
        };

        let schema = engine.schema_for_query(&query);
        let entity_paths: Vec<_> = schema
            .components
            .iter()
            .map(|c| c.entity_path.clone())
            .collect();

        assert!(entity_paths.contains(&EntityPath::from("/a")));
        assert!(!entity_paths.contains(&EntityPath::from("/b")));

        Ok(())
    }

    #[test]
    fn selected_schema_for_query_none_selection_matches_view_contents() -> anyhow::Result<()> {
        let store = ChunkStoreHandle::new(build_two_entity_store()?);
        let engine = QueryEngine::from_store(store);

        let query = QueryExpression::default();

        assert_eq!(
            engine.selected_schema_for_query(&query),
            engine.schema_for_query(&query).indices_and_components(),
        );

        Ok(())
    }

    #[test]
    fn selected_schema_for_query_synthesizes_placeholders_for_misses() -> anyhow::Result<()> {
        use re_sorbet::{ColumnSelector, ComponentColumnSelector, TimeColumnSelector};

        let store = ChunkStoreHandle::new(build_two_entity_store()?);
        let engine = QueryEngine::from_store(store);

        let query = QueryExpression {
            selection: Some(vec![
                // `RowId` is always a miss: `indices_and_components()` excludes it from
                // `view_contents`, per the `TODO(#9922)` on `ChunkColumnDescriptors`.
                ColumnSelector::RowId,
                // Unknown timeline: never present in `view_contents`.
                ColumnSelector::Time(TimeColumnSelector::from(re_log_types::TimelineName::from(
                    "does_not_exist",
                ))),
                // Unknown component on a real entity.
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: EntityPath::from("/a"),
                    component: "DoesNotExist".to_owned(),
                }),
            ]),
            ..Default::default()
        };

        let selected = engine.selected_schema_for_query(&query);
        assert_eq!(selected.len(), 3);

        assert!(matches!(selected[0], ColumnDescriptor::RowId(_)));
        assert!(matches!(selected[1], ColumnDescriptor::Time(_)));
        match &selected[2] {
            ColumnDescriptor::Component(c) => {
                assert_eq!(c.store_datatype, arrow::datatypes::DataType::Null);
            }
            other => panic!("expected a placeholder component descriptor, got {other:?}"),
        }

        Ok(())
    }

    /// `selected_schema_for_query` is documented as "computed cheaply… but must match"
    /// what a real [`crate::QueryHandle`] resolves at init time. Pin that down.
    #[test]
    fn selected_schema_for_query_matches_query_handle_selected_contents() -> anyhow::Result<()> {
        use re_sorbet::{ColumnSelector, ComponentColumnSelector};

        let store = ChunkStoreHandle::new(build_two_entity_store()?);
        let engine = QueryEngine::from_store(store);

        let query = QueryExpression {
            selection: Some(vec![
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: EntityPath::from("/a"),
                    component: MyPoints::descriptor_points().component.to_string(),
                }),
                // Miss: `/b`'s actual component is `Color`, not `Points`.
                ColumnSelector::Component(ComponentColumnSelector {
                    entity_path: EntityPath::from("/b"),
                    component: MyPoints::descriptor_points().component.to_string(),
                }),
            ]),
            ..Default::default()
        };

        let from_engine = engine.selected_schema_for_query(&query);
        let from_handle: Vec<_> = engine
            .query(query)
            .selected_contents()
            .iter()
            .map(|(_, descr)| descr.clone())
            .collect();

        assert_eq!(from_engine, from_handle);

        Ok(())
    }

    #[test]
    fn iter_entity_paths_sorted_orders_lexically_regardless_of_insertion_order()
    -> anyhow::Result<()> {
        let store = build_store_with_entities(["/z", "/a", "/m/n"])?;
        let engine = QueryEngine::from_store(ChunkStoreHandle::new(store));
        let filter = EntityPathFilter::parse_forgiving("+ /**");

        let entity_paths: Vec<_> = engine.iter_entity_paths_sorted(&filter).collect();

        assert_eq!(
            entity_paths,
            vec![
                EntityPath::from("/a"),
                EntityPath::from("/m/n"),
                EntityPath::from("/z"),
            ]
        );

        Ok(())
    }

    #[test]
    fn iter_entity_paths_sorted_respects_filter() -> anyhow::Result<()> {
        let store = build_store_with_entities(["/z", "/a", "/m/n"])?;
        let engine = QueryEngine::from_store(ChunkStoreHandle::new(store));

        let matching_filter = EntityPathFilter::parse_forgiving("+ /m/**");
        assert_eq!(
            engine
                .iter_entity_paths_sorted(&matching_filter)
                .collect::<Vec<_>>(),
            vec![EntityPath::from("/m/n")],
        );

        let never_matching_filter = EntityPathFilter::parse_forgiving("");
        assert_eq!(
            engine
                .iter_entity_paths_sorted(&never_matching_filter)
                .count(),
            0,
        );

        Ok(())
    }

    #[test]
    fn schema_on_empty_store_returns_empty_descriptors() {
        let store = ChunkStoreHandle::new(ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        ));
        let engine = QueryEngine::from_store(store);

        let schema = engine.schema();
        assert!(schema.indices.is_empty());
        assert!(schema.components.is_empty());
    }
}
