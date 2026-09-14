//! Core HDF5-to-chunk loading logic for Rerun.
//!
//! Reads an HDF5 file into a lazy stream of Rerun chunks: each HDF5 group maps
//! to an entity, each leaf dataset to a component, with a single file-wide
//! timeline (a designated 1-D index dataset, or a synthesized `row_index`
//! sequence). HDF5 attributes are emitted as static components under a
//! dedicated `__hdf5_properties` entity mirroring the source layout.
//!
//! Scope is raw HDF5 → typed Arrow columns only — no semantic interpretation
//! into archetypes (that is a downstream lens concern).

mod config;
mod convert;
mod error;
mod load;
mod plan;
mod streaming;
mod walk;

pub use config::{Hdf5Config, IndexColumn, IndexType, TimeUnit};
pub use convert::DatasetDtype;
pub use error::Hdf5Error;
pub use load::{
    DatasetInfo, list_datasets, list_groups, load_hdf5, load_hdf5_from_bytes, read_attributes,
    validate_layout,
};

// The attribute value type returned by `read_attributes`.
pub use hdf5_pure::AttrValue;
