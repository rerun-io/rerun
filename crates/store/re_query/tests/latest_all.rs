// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use re_chunk::RowId;
use re_chunk_store::external::re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreSubscriber as _, LatestAtQuery};
use re_log_types::EntityPath;
use re_log_types::build_frame_nr;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_query::QueryCache;

// ---

/// Test that `latest_all` works the same as `latest_at` when there's only one row per timestamp.
#[test]
fn simple_query() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let row_id1 = RowId::new();
    let points1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk = Chunk::builder(entity_path)
        .with_archetype(row_id1, timepoint, &MyPoints::new(points1.clone()))
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = LatestAtQuery::new(*timepoint[0].0.name(), timepoint[0].1);
    let component_points = MyPoints::descriptor_points().component;

    let latest_all_results = caches.latest_all(&query, &entity_path.into(), [component_points]);
    assert!(latest_all_results.missing_virtual.is_empty());

    // With exactly one row per component, try_as_latest_at should succeed
    let latest_at_results = latest_all_results.try_as_latest_at().unwrap();
    let points: Vec<MyPoint> = latest_at_results.component_batch(component_points).unwrap();
    similar_asserts::assert_eq!(points1, points);

    re_log::debug_assert_eq!(
        caches.latest_at(&query, &entity_path.into(), [component_points]),
        latest_at_results,
    );
}

/// Test that `latest_all` returns multiple rows when they are logged at the same timestamp.
/// This is the key difference from `latest_at`.
#[test]
fn multiple_rows_same_timestamp() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Log multiple colors at the same timestamp (simulating multiple transforms, etc.)
    let row_id1 = RowId::new();
    let colors1 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk1 = Chunk::builder(entity_path)
        .with_archetype(
            row_id1,
            timepoint,
            &MyPoints::update_fields().with_colors(colors1.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk1));

    let row_id2 = RowId::new();
    let colors2 = vec![MyColor::from_rgb(0, 255, 0)];
    let chunk2 = Chunk::builder(entity_path)
        .with_archetype(
            row_id2,
            timepoint,
            &MyPoints::update_fields().with_colors(colors2.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk2));

    let row_id3 = RowId::new();
    let colors3 = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk3 = Chunk::builder(entity_path)
        .with_archetype(
            row_id3,
            timepoint,
            &MyPoints::update_fields().with_colors(colors3.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk3));

    let query = LatestAtQuery::new(*timepoint[0].0.name(), timepoint[0].1);
    let component_colors = MyPoints::descriptor_colors().component;

    // latest_at should only return the last one
    let latest_at_results = caches.latest_at(&query, &entity_path.into(), [component_colors]);
    let latest_at_colors: Vec<MyColor> =
        latest_at_results.component_batch(component_colors).unwrap();
    assert_eq!(latest_at_colors.len(), 1);

    // latest_all should return all three
    let latest_all_results = caches.latest_all(&query, &entity_path.into(), [component_colors]);
    assert!(latest_all_results.missing_virtual.is_empty());

    // With multiple rows, try_as_latest_at should return None
    assert!(latest_all_results.try_as_latest_at().is_none());

    let component_results = &latest_all_results.components[&component_colors];
    let all_colors: Vec<MyColor> = component_results
        .iter_component_batches(component_colors)
        .flatten()
        .collect();

    assert_eq!(all_colors, [colors1[0], colors2[0], colors3[0]]);
}

/// Test that `latest_all` handles the case where no data exists for the query.
#[test]
fn empty_query() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let caches = QueryCache::new(store.clone());

    let entity_path: EntityPath = "point".into();
    let timepoint = [build_frame_nr(123)];

    let query = LatestAtQuery::new(*timepoint[0].0.name(), timepoint[0].1);
    let component_points = MyPoints::descriptor_points().component;

    let results = caches.latest_all(&query, &entity_path, [component_points]);

    assert!(results.missing_virtual.is_empty());
    assert!(results.components.is_empty());
}

/// Test that `latest_all` correctly handles querying at a time before any data.
#[test]
fn query_before_data() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let row_id1 = RowId::new();
    let points1 = vec![MyPoint::new(1.0, 2.0)];
    let chunk = Chunk::builder(entity_path)
        .with_archetype(row_id1, timepoint, &MyPoints::new(points1))
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    // Query at an earlier time
    let query_time = [build_frame_nr(100)];
    let query = LatestAtQuery::new(*query_time[0].0.name(), query_time[0].1);
    let component_points = MyPoints::descriptor_points().component;

    let results = caches.latest_all(&query, &entity_path.into(), [component_points]);

    assert!(results.missing_virtual.is_empty());
    assert!(results.components.is_empty());
}

/// Test that `into_latest_at` extracts the row with the highest `RowId` from multiple rows.
#[test]
fn into_latest_at() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Log multiple colors at the same timestamp with deterministic RowIds
    let row_id1 = RowId::from_u128(1);
    let colors1 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk1 = Chunk::builder(entity_path)
        .with_archetype(
            row_id1,
            timepoint,
            &MyPoints::update_fields().with_colors(colors1.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk1));

    let row_id2 = RowId::from_u128(2);
    let colors2 = vec![MyColor::from_rgb(0, 255, 0)];
    let chunk2 = Chunk::builder(entity_path)
        .with_archetype(
            row_id2,
            timepoint,
            &MyPoints::update_fields().with_colors(colors2.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk2));

    let row_id3 = RowId::from_u128(3);
    let colors3 = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk3 = Chunk::builder(entity_path)
        .with_archetype(
            row_id3,
            timepoint,
            &MyPoints::update_fields().with_colors(colors3.clone()),
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk3));

    let query = LatestAtQuery::new(*timepoint[0].0.name(), timepoint[0].1);
    let component_colors = MyPoints::descriptor_colors().component;

    // latest_all returns all three rows
    let latest_all_results = caches.latest_all(&query, &entity_path.into(), [component_colors]);
    assert!(latest_all_results.missing_virtual.is_empty());
    assert!(latest_all_results.try_as_latest_at().is_none()); // Multiple rows

    // into_latest_at should give us only the row with the highest RowId (row_id3)
    let latest_at_results = latest_all_results.into_latest_at();
    let colors: Vec<MyColor> = latest_at_results.component_batch(component_colors).unwrap();

    // Should return colors3 (the one with highest RowId)
    assert_eq!(colors, colors3);
    assert_eq!(
        latest_at_results.component_row_id(component_colors),
        Some(row_id3)
    );
}

// ---

fn insert_and_react(store: &mut ChunkStore, caches: &mut QueryCache, chunk: &Arc<Chunk>) {
    caches.on_events(&store.insert_chunk(chunk).unwrap());
}
