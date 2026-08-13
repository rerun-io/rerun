//! Every component column of a [`re_chunk::Chunk`] describes its outer list the same way,
//! whatever the producer declared.
//!
//! `ChunkStore` hands out `ComponentColumnDescriptor`s derived from that column, and readers
//! build arrow schemas from those descriptors while taking the data straight from the chunk. The
//! two only agree if the chunk itself is canonical — see
//! <https://github.com/rerun-io/rerun/issues/12887>, where an MCAP-produced
//! `List(non-null List(non-null UInt8))` blob column made `RecordBatch::try_new` reject its own
//! rows.

use std::sync::Arc;

use arrow::array::{Array as _, ListArray, UInt8Array};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field};
use re_chunk::{Chunk, ChunkComponents, ChunkId};
use re_log_types::EntityPath;
use re_types_core::ComponentDescriptor;

/// A `Blob`-shaped column with a non-nullable outer item (i.e. non-canonical):
/// `List(non-null List(non-null UInt8))`.
fn non_canonical_blob_column() -> ListArray {
    let bytes = Arc::new(UInt8Array::from(vec![1u8, 2, 3]));
    let blob = Arc::new(ListArray::new(
        Arc::new(Field::new_list_field(DataType::UInt8, false)),
        OffsetBuffer::from_lengths([2, 1]),
        bytes,
        None,
    ));
    ListArray::new(
        Arc::new(Field::new_list_field(blob.data_type().clone(), false)),
        OffsetBuffer::from_lengths([1, 1]),
        blob,
        None,
    )
}

/// The canonical shape of the same column: only the outer item's nullability differs. The inner
/// `List(non-null UInt8)` is `Blob`'s own datatype and must survive untouched.
fn canonical_blob_datatype() -> DataType {
    DataType::List(Arc::new(Field::new_list_field(
        DataType::List(Arc::new(Field::new_list_field(DataType::UInt8, false))),
        true,
    )))
}

#[test]
fn chunk_new_canonicalizes_outer_list_field() {
    let column = non_canonical_blob_column();
    let component = ComponentDescriptor::partial("blob");

    let components: ChunkComponents =
        std::iter::once((component.clone(), column.clone())).collect();
    let chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        EntityPath::from("blobs"),
        Default::default(), // static
        components,
    )
    .unwrap();

    let stored = chunk
        .components()
        .get_array(component.component)
        .expect("component column");

    assert_eq!(stored.data_type(), &canonical_blob_datatype());

    // Only the field was rewritten: the data is the same, and the chunk is still valid.
    assert_eq!(stored.values(), column.values());
    assert_eq!(stored.len(), column.len());
    chunk.sanity_check().unwrap();
}

/// Columns added after construction go through the same normalization.
#[test]
fn add_component_canonicalizes_outer_list_field() {
    let component = ComponentDescriptor::partial("blob");
    let mut chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        EntityPath::from("blobs"),
        Default::default(),
        std::iter::once((component.clone(), non_canonical_blob_column())).collect(),
    )
    .unwrap();

    let other = ComponentDescriptor::partial("other_blob");
    chunk
        .add_component(re_types_core::SerializedComponentColumn::new(
            non_canonical_blob_column(),
            other.clone(),
        ))
        .unwrap();

    assert_eq!(
        chunk
            .components()
            .get_array(other.component)
            .expect("component column")
            .data_type(),
        &canonical_blob_datatype(),
    );
}

/// The canonical field survives a round-trip through the chunk's arrow encoding, which is what
/// `.rrd` files and gRPC carry.
#[test]
fn canonical_outer_list_field_survives_record_batch_roundtrip() {
    let component = ComponentDescriptor::partial("blob");
    let chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        EntityPath::from("blobs"),
        Default::default(),
        std::iter::once((component.clone(), non_canonical_blob_column())).collect(),
    )
    .unwrap();

    let batch = chunk.to_record_batch().unwrap();
    let roundtripped = Chunk::from_chunk_record_batch(&batch).unwrap();

    assert_eq!(
        roundtripped
            .components()
            .get_array(component.component)
            .expect("component column")
            .data_type(),
        &canonical_blob_datatype(),
    );
}
