//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.
//!
//! An arrow record batch that follows a specific schema is called a [`SorbetBatch`].
//!
//! Some [`SorbetBatch`]es has even more constrained requirements, such as [`ChunkBatch`] and `DataframeBatch`.
//! * Every [`ChunkBatch`] is a [`SorbetBatch`].
//! * Every `DataframeBatch` is a [`SorbetBatch`].
//!
//! NOTE: `DataframeBatch` has not yet been implemented.
//!
//! Each batch type has a matching schema type:
//! * [`SorbetBatch`] has a [`SorbetSchema`]
//! * [`ChunkBatch`] has a [`ChunkSchema`]
//! * `DataframeBatch` will have a `DataframeSchema`

mod chunk_batch;
mod chunk_schema;
mod column_descriptor;
mod column_descriptor_ref;
mod column_kind;
mod component_column_descriptor;
mod error;
mod index_column_descriptor;
mod ipc;
mod metadata;
mod migration;
mod row_id_column_descriptor;
mod selectors;
mod sorbet_batch;
mod sorbet_columns;
mod sorbet_schema;

pub use self::{
    chunk_batch::{ChunkBatch, MismatchedChunkSchemaError},
    chunk_schema::ChunkSchema,
    column_descriptor::{ColumnDescriptor, ColumnError},
    column_descriptor_ref::ColumnDescriptorRef,
    column_kind::{ColumnKind, UnknownColumnKind},
    component_column_descriptor::ComponentColumnDescriptor,
    error::SorbetError,
    index_column_descriptor::{IndexColumnDescriptor, UnsupportedTimeType},
    ipc::{ipc_from_schema, schema_from_ipc},
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
    migration::migrate_record_batch,
    row_id_column_descriptor::{RowIdColumnDescriptor, WrongDatatypeError},
    selectors::{
        ColumnSelector, ColumnSelectorParseError, ComponentColumnSelector, TimeColumnSelector,
    },
    sorbet_batch::SorbetBatch,
    sorbet_columns::{ColumnSelectorResolveError, SorbetColumnDescriptors},
    sorbet_schema::SorbetSchema,
};

/// The type of [`SorbetBatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchType {
    /// Data for one entity
    Chunk,

    /// Potentially multiple entities
    Dataframe,
}
