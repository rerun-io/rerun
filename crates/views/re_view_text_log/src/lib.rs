//! Rerun `TextLog` View
//!
//! A View that shows `TextLog` entries in a table and scrolls with the active time.

mod text_log_index_cache;
mod text_log_level_cache;
mod text_log_navigation;
mod text_log_query;
mod text_log_source;
mod text_log_span_index;
mod view_class;
mod visualizer_system;

pub use view_class::TextView;
