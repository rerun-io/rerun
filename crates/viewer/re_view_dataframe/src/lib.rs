//! Rerun `Data` View
//!
//! A View that shows the data contained in entities in a table.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod dataframe_ui;

mod expanded_rows;
mod view_class;
mod view_query;
mod visualizer_system;

pub use view_class::DataframeView;
pub use view_query::Query;
