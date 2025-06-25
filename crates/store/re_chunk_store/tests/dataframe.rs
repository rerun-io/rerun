use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimelineName};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, QueryExpression, StaticColumnSelection, TimeInt,
    ViewContentsSelector,
};
use re_log_types::{
    EntityPath, build_frame_nr,
    example_components::{MyColor, MyLabel, MyPoint, MyPoints},
};
use re_sorbet::ChunkColumnDescriptors;

#[test]
/// Tests whether the store has the expected schema after populating it with a chunk.
fn schema() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
            MyPoints::descriptor_indicator(),
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
        include_indicator_columns: false,
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
