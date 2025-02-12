//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.
//!
//! An arrow record batch that follows a specific schema is called a `SorbetBatch`.
//!
//! Some `SorbetBatch`es has even more constrained requirements, such as [`ChunkBatch`] and `DataframeBatch`.
//! * Every [`ChunkBatch`] is a `SorbetBatch`.
//! * Every `DataframeBatch` is a `SorbetBatch`.

mod chunk_batch;
mod chunk_schema;
mod column_descriptor;
mod column_kind;
mod component_column_descriptor;
mod index_column_descriptor;
mod ipc;
mod metadata;
mod row_id_column_descriptor;
mod sorbet_columns;
mod sorbet_schema;

pub use self::{
    chunk_batch::{ChunkBatch, MismatchedChunkSchemaError},
    chunk_schema::ChunkSchema,
    column_descriptor::{ColumnDescriptor, ColumnError},
    column_kind::ColumnKind,
    component_column_descriptor::ComponentColumnDescriptor,
    index_column_descriptor::{IndexColumnDescriptor, UnsupportedTimeType},
    ipc::{ipc_from_schema, schema_from_ipc},
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
    row_id_column_descriptor::{RowIdColumnDescriptor, WrongDatatypeError},
    sorbet_columns::SorbetColumnDescriptors,
    sorbet_schema::{InvalidSorbetSchema, SorbetSchema},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchType {
    /// Data for one entity
    Chunk,

    /// Potentially multiple entities
    Dataframe,
}
