//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

mod add_server_modal;
mod context;
mod dataset_ui;
mod entries;
mod requested_object;
mod servers;

pub use entries::{dataset_and_its_recordings_ui, sort_datasets, EntryKind, SortDatasetsResults};
pub use servers::RedapServers;
