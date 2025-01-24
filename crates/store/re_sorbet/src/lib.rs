//! Rerun arrow metadata definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.

mod data_column_schema;
mod index_column_schema;
mod metadata;

pub use self::{
    data_column_schema::ComponentColumnDescriptor,
    index_column_schema::TimeColumnDescriptor,
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
};
