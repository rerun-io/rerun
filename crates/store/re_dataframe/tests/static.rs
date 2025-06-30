use std::sync::Arc;

use arrow::array::{ListArray, RecordBatchIterator, StringArray, UInt32Array};
use re_chunk::{ArrowArray as _, Chunk, RowId, TimePoint};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, QueryExpression};
use re_dataframe::QueryEngine;
use re_log_types::EntityPath;
use re_types::AsComponents as _;
use re_types::{AnyValues, ComponentDescriptor};

#[test]
fn query_static_columns() -> anyhow::Result<()> {
    re_log::setup_logging();

    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let any_values = AnyValues::default()
        .with_field("yak", Arc::new(StringArray::from(vec!["yuk"])))
        .with_field("foo", Arc::new(StringArray::from(vec!["bar"])))
        .with_field("baz", Arc::new(UInt32Array::from(vec![42u32])));

    let entity_path = EntityPath::from("test");

    let chunk0 = Chunk::builder(entity_path.clone())
        .with_serialized_batches(
            RowId::new(),
            TimePoint::default(),
            any_values.as_serialized_batches(),
        )
        .build()?;

    store.write().insert_chunk(&Arc::new(chunk0))?;

    let engine = QueryEngine::from_store(store);

    let query_expr = QueryExpression {
        view_contents: None,
        include_semantically_empty_columns: false,
        include_indicator_columns: false,
        include_tombstone_columns: false,
        include_static_columns: re_chunk_store::StaticColumnSelection::Both,
        filtered_index: None,
        filtered_index_range: None,
        filtered_index_values: None,
        using_index_values: None,
        filtered_is_not_null: None,
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        selection: None,
    };

    let query_handle = engine.query(query_expr);

    let components = &query_handle.view_contents().components;

    assert_eq!(
        components
            .iter()
            .map(|column| (column.component_descriptor(), column.is_static))
            .collect::<Vec<_>>(),
        vec![
            (ComponentDescriptor::partial("baz"), true),
            (ComponentDescriptor::partial("foo"), true),
            (ComponentDescriptor::partial("yak"), true),
        ]
    );

    let schema = query_handle.schema().clone();

    let mut reader = RecordBatchIterator::new(query_handle.into_batch_iter().map(Ok), schema);

    let batch = reader.next().expect("there should be at least one batch")?;

    eprintln!("{batch:#?}");

    assert_eq!(batch.num_columns(), 3);
    assert_eq!(batch.num_rows(), 1);

    let baz = batch
        .column(0)
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap()
        .value(0);
    let baz = baz.as_any().downcast_ref::<UInt32Array>().unwrap();
    assert_eq!(baz.value(0), 42);

    let foo = batch
        .column(1)
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap()
        .value(0);
    let foo = foo.as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(foo.value(0), "bar");

    let yak = batch
        .column(2)
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap()
        .value(0);
    let yak = yak.as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(yak.value(0), "yuk");

    Ok(())
}
