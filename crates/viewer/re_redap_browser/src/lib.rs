//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

mod add_server_modal;
mod dataset_ui;
//mod collections;
mod context;
mod entries;
mod requested_object;
mod servers;

pub use servers::RedapServers;
