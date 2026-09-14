//! This crate implements the UI for the blueprint tree in the left panel.

mod blueprint_tree;

#[cfg(feature = "testing")]
pub mod data;

#[cfg(not(feature = "testing"))]
pub(crate) mod data;
mod data_result_node_or_path;

pub use blueprint_tree::BlueprintTree;
