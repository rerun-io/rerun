use std::sync::Arc;

use arrow::array::{StringArray, UInt32Array};
use re_chunk::{Chunk, RowId, TimePoint, TimelineName};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, QueryExpression, StaticColumnSelection, TimeInt,
    ViewContentsSelector,
};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_log_types::{EntityPath, build_frame_nr};
use re_sdk_types::{AnyValues, AsComponents as _};
use re_sorbet::ChunkColumnDescriptors;

#[test]
/// Tests whether the store has the expected schema after populating it with a chunk.
fn schema() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("log");
    let frame1 = TimeInt::new_temporal(1);

    let row_id1 = RowId::new();
    let (points1, colors1, labels1) = (
        MyPoint::from_iter(0..10),
        MyColor::from_iter(0..3),
        vec![MyLabel("test123".to_owned())],
    );
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_archetype(
            row_id1,
            [build_frame_nr(frame1)],
            &MyPoints::new(points1)
                .with_colors(colors1)
                .with_labels(labels1),
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    store.insert_chunk(&chunk1)?;

    let ChunkColumnDescriptors { components, .. } = store.schema();

    assert_eq!(
        components
            .iter()
            .map(|column| column.component_descriptor())
            .collect::<Vec<_>>(),
        // It's important that the returned descriptors are in lexicographical order, as we
        // want the schema to be deterministic between calls.
        //
        // The lexicographical order is defined by the component descriptors. Note that the
        // indicator plays a special role here, because it has the archetype field set to
        // `None`. Also, indicators will be removed soon anyways.
        vec![
            MyPoints::descriptor_colors(),
            MyPoints::descriptor_labels(),
            MyPoints::descriptor_points(),
        ]
    );

    Ok(())
}

#[test]
/// Tests whether the `schema_for_query` for a given query has the expected contents.
fn schema_for_query() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("log");
    let frame1 = TimeInt::new_temporal(1);

    let row_id1 = RowId::new();
    let (points1, colors1, labels1) = (
        MyPoint::from_iter(0..10),
        MyColor::from_iter(0..3),
        vec![MyLabel("test123".to_owned())],
    );
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_archetype(
            row_id1,
            [build_frame_nr(frame1)],
            &MyPoints::new(points1)
                .with_colors(colors1)
                .with_labels(labels1),
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    store.insert_chunk(&chunk1)?;

    let query = QueryExpression {
        view_contents: Some(ViewContentsSelector::from_iter([(
            entity_path,
            Some(
                [
                    MyPoints::descriptor_colors().component,
                    MyPoints::descriptor_labels().component,
                ]
                .into(),
            ),
        )])),
        include_semantically_empty_columns: false,
        include_tombstone_columns: false,
        include_static_columns: StaticColumnSelection::Both,
        filtered_index: Some(TimelineName::new("frame_nr")),
        filtered_index_range: None,
        filtered_index_values: None,
        using_index_values: None,
        filtered_is_not_null: None,
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        selection: None,
    };

    let ChunkColumnDescriptors { components, .. } = store.schema_for_query(&query);

    assert_eq!(
        components
            .iter()
            .map(|column| column.component_descriptor())
            .collect::<Vec<_>>(),
        // The following should be in lexicographical order!
        vec![MyPoints::descriptor_colors(), MyPoints::descriptor_labels(),]
    );

    Ok(())
}

#[test]
fn schema_static_columns() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let any_values = AnyValues::default()
        .with_component_from_data("yak", Arc::new(StringArray::from(vec!["yuk"])))
        .with_component_from_data("foo", Arc::new(StringArray::from(vec!["bar"])))
        .with_component_from_data("baz", Arc::new(UInt32Array::from(vec![42u32])));

    let entity_path = EntityPath::from("test");

    let chunk0 = Chunk::builder(entity_path.clone())
        .with_serialized_batches(
            RowId::new(),
            TimePoint::default(),
            any_values.as_serialized_batches(),
        )
        .build()?;

    store.insert_chunk(&Arc::new(chunk0))?;

    let frame1 = TimeInt::new_temporal(1);
    let chunk1 = Chunk::builder(entity_path)
        .with_archetype(
            RowId::new(),
            [build_frame_nr(frame1)],
            &MyPoints::new(MyPoint::from_iter(0..1)),
        )
        .build()?;

    store.insert_chunk(&Arc::new(chunk1))?;

    let query = QueryExpression {
        view_contents: None,
        include_semantically_empty_columns: false,
        include_tombstone_columns: false,
        include_static_columns: StaticColumnSelection::Both,
        filtered_index: None,
        filtered_index_range: None,
        filtered_index_values: None,
        using_index_values: None,
        filtered_is_not_null: None,
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        selection: None,
    };

    // Both
    {
        let ChunkColumnDescriptors { components, .. } = store.schema_for_query(&query);
        let both_static_and_non_static = components
            .iter()
            .map(|column| column.component_descriptor().component)
            .collect::<Vec<_>>();

        insta::assert_debug_snapshot!(both_static_and_non_static);
    }

    // Static
    {
        let query = QueryExpression {
            include_static_columns: StaticColumnSelection::StaticOnly,
            ..query.clone()
        };

        let ChunkColumnDescriptors { components, .. } = store.schema_for_query(&query);
        let static_only = components
            .iter()
            .map(|column| column.component_descriptor().component)
            .collect::<Vec<_>>();

        insta::assert_debug_snapshot!(static_only);
    }

    // Non-static
    {
        let query = QueryExpression {
            include_static_columns: StaticColumnSelection::NonStaticOnly,
            ..query
        };

        let ChunkColumnDescriptors { components, .. } = store.schema_for_query(&query);
        let non_static_only = components
            .iter()
            .map(|column| column.component_descriptor().component)
            .collect::<Vec<_>>();

        insta::assert_debug_snapshot!(non_static_only);
    }

    Ok(())
}
