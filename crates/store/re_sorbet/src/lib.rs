//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.

mod column_schema;
mod data_column_schema;
mod index_column_schema;
mod ipc;
mod metadata;
mod row_id_column_schema;

pub use self::{
    column_schema::{ColumnDescriptor, ColumnError},
    data_column_schema::ComponentColumnDescriptor,
    index_column_schema::{TimeColumnDescriptor, UnsupportedTimeType},
    ipc::{ipc_from_schema, schema_from_ipc},
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
    row_id_column_schema::{RowIdColumnDescriptor, WrongDatatypeError},
};
