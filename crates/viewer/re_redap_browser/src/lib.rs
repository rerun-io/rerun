//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

mod add_server_modal;
mod context;
mod dataset_ui;
mod entries;
mod requested_object;
mod servers;

pub use servers::RedapServers;
