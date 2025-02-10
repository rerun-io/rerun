//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.
//!
//! An arrow record batch needs to follow a specific schema to be be compatible with Rerun,
//! and that schema is defined in [`ChunkSchema`].
//! If a record batch matches the schema, it can be converted to a [`ChunkBatch`].

mod chunk_batch;
mod chunk_schema;
mod column_schema;
mod data_column_schema;
mod index_column_schema;
mod ipc;
mod metadata;
mod row_id_column_schema;

pub use self::{
    chunk_batch::{ChunkBatch, MismatchedChunkSchemaError},
    chunk_schema::{ChunkSchema, InvalidChunkSchema},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchType {
    /// Data for one entity
    Chunk,

    /// Potentially multiple entities
    Dataframe,
}
