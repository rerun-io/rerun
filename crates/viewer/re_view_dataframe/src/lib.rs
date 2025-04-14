//! Rerun `Data` View
//!
//! A View that shows the data contained in entities in a table.

mod dataframe_ui;

mod expanded_rows;
mod view_class;
mod view_query;
mod visualizer_system;

pub use view_class::DataframeView;

#[cfg(feature = "testing")]
pub use view_query::Query;
