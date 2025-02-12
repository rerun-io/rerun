//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.
//!
//! An arrow record batch needs to follow a specific schema to be compatible with Rerun,
//! and that schema is defined in [`ChunkSchema`].
//! If a record batch matches the schema, it can be converted to a [`ChunkBatch`].

mod chunk_batch;
mod chunk_schema;
mod column_descriptor;
mod component_column_descriptor;
mod index_column_descriptor;
mod ipc;
mod metadata;
mod row_id_column_descriptor;

pub use self::{
    chunk_batch::{ChunkBatch, MismatchedChunkSchemaError},
    chunk_schema::{ChunkSchema, InvalidChunkSchema},
    column_descriptor::{ColumnDescriptor, ColumnError},
    component_column_descriptor::ComponentColumnDescriptor,
    index_column_descriptor::{IndexColumnDescriptor, UnsupportedTimeType},
    ipc::{ipc_from_schema, schema_from_ipc},
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
    row_id_column_descriptor::{RowIdColumnDescriptor, WrongDatatypeError},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchType {
    /// Data for one entity
    Chunk,

    /// Potentially multiple entities
    Dataframe,
}
