//! Rerun `Data` View
//!
//! A View that shows the data contained in entities in a table.

mod dataframe_ui;
//TODO(ab): this should be moved somewhere else
pub mod display_record_batch;
mod expanded_rows;
mod view_class;
mod view_query;
mod visualizer_system;

pub use view_class::DataframeView;

#[cfg(feature = "testing")]
pub use view_query::Query;
