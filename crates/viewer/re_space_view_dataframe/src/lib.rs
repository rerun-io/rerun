//! Rerun `Data` Space View
//!
//! A Space View that shows the data contained in entities in a table.

mod dataframe_ui;
mod display_record_batch;
mod latest_at_table;
mod query_kind;
mod space_view_class;
mod table_ui;
mod time_range_table;
mod utils;
mod view_query;
mod visualizer_system;

pub use space_view_class::DataframeSpaceView;
