//! Chunks and SDK query caches, to check that chunk candidates resolve like the full data.

use std::sync::Arc;

use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ChunkStoreSubscriber as _};
use re_log_types::example_components::MyPoint;
use re_log_types::{StoreId, StoreKind, TimeInt, TimePoint, build_frame_nr};
use re_query::QueryCache;
use re_types_core::{ComponentBatch, ComponentDescriptor};

/// One row of a [`point_chunk`]: its row id, its time on `frame_nr` or `None` for static, and one
/// `MyPoint` value per component it holds.
pub type PointRow<'a> = (RowId, Option<i64>, &'a [(&'a ComponentDescriptor, f32)]);

/// A chunk `id` on `entity` with one row per entry of `rows`.
///
/// A row holds the point `(value, value)` for each of its components. The chunk holds nulls where
/// a row lacks a component that another row has.
pub fn point_chunk(id: ChunkId, entity: &str, rows: &[PointRow<'_>]) -> Chunk {
    let mut builder = Chunk::builder_with_id(id, entity);
    for (row_id, time, values) in rows {
        let timepoint = time.map_or(TimePoint::STATIC, |time| {
            [build_frame_nr(TimeInt::new_temporal(time))].into()
        });
        let points: Vec<[MyPoint; 1]> = values
            .iter()
            .map(|(_, value)| [MyPoint::new(*value, *value)])
            .collect();
        builder = builder.with_component_batches(
            *row_id,
            timepoint,
            std::iter::zip(*values, &points).map(|((descriptor, _), points)| {
                ((*descriptor).clone(), points as &dyn ComponentBatch)
            }),
        );
    }
    builder.build().expect("valid point chunk")
}

/// An SDK query cache over `chunks`.
pub fn query_cache<'a>(chunks: impl IntoIterator<Item = &'a Chunk>) -> QueryCache {
    let store = ChunkStoreHandle::new(ChunkStore::new(
        StoreId::random(StoreKind::Recording, "samples_resolution"),
        ChunkStoreConfig::ALL_DISABLED,
    ));
    let mut cache = QueryCache::new(store.clone());
    for chunk in chunks {
        let events = store
            .write()
            .insert_chunk(&Arc::new(chunk.clone()))
            .expect("fixture chunk must insert");
        cache.on_events(&events);
    }
    cache
}
