//! Rerun `Data` View
//!
//! A View that shows the data contained in entities in a table.

mod dataframe_ui;

mod expanded_rows;
mod view_class;
mod view_query;

pub use view_class::DataframeView;
pub use view_query::Query;
