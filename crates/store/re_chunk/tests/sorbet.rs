//! Tests the assumptions of [`re_sorbet`].
//!
//! Ideally these tests would live there, but this is currently not possible the way
//! our dependencies are structured (cyclic dependency).

use arrow::array::{ArrayRef, Int32Builder, ListBuilder};
use re_log_types::{EntityPath, TimePoint};
use re_types_core::{ChunkId, ComponentDescriptor, RowId};

#[test]
fn sorbet_version_presence() {
    let mut array_builder = ListBuilder::new(Int32Builder::new());

    array_builder.append_value([Some(1), Some(2), Some(3)]);

    let array = std::sync::Arc::new(array_builder.finish());
    let chunk = re_chunk::ChunkBuilder::new(ChunkId::new(), EntityPath::root())
        .with_row(
            RowId::new(),
            TimePoint::STATIC,
            [(ComponentDescriptor::partial("test"), array as ArrayRef)],
        )
        .build()
        .unwrap();

    let rb = chunk.to_record_batch().unwrap();
    let schema = rb.schema();
    let metadata = schema.metadata();

    println!("{metadata:?}");
    assert!(
        metadata.contains_key("sorbet:version"),
        "migration code in `re_sorbet` relies on `sorbet:version` to be present"
    );
}
