//! Rich table widget over `datafusion`.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod filters;
mod grid_view;
mod header_tooltip;
mod re_table;
pub mod re_table_utils;
mod requested_object;
mod streaming_cache;
mod table_blueprint;
mod table_selection;

pub use self::datafusion_table_widget::{DataFusionTableWidget, TableStatus};
pub use self::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
// for testing purposes
pub use self::filters::{
    ColumnFilter, ComparisonOperator, Filter, FloatFilter, IntFilter, NonNullableBooleanFilter,
    Nullability, NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
};
pub use self::header_tooltip::column_header_tooltip_ui;
pub use self::requested_object::RequestedObject;
pub use self::streaming_cache::{CacheState, StreamingCacheTableProvider};
pub use self::table_blueprint::{
    ColumnBlueprint, SortBy, SortDirection, TableBlueprint, default_display_name_for_column,
};

/// Arrow field metadata keys for configuring table grid view behavior.
///
/// These are read from [`arrow::datatypes::Field::metadata`] and populate the corresponding [`TableBlueprint`] fields.
pub mod experimental_field_metadata {
    /// Mark a boolean column as the flag/annotation toggle column.
    ///
    /// Set to `"true"` on a boolean field's metadata.
    pub const IS_FLAG_COLUMN: &str = "rerun:is_flag_column";

    /// Mark a column as the card title in grid view.
    ///
    /// Set to `"true"` on a field's metadata. If no column is marked, the first visible string column is used.
    pub const IS_GRID_VIEW_CARD_TITLE: &str = "rerun:is_grid_view_card_title";
}

/// Create a blocking channel on native, and an unbounded channel on web.
fn create_channel<T>(
    size: usize,
) -> (
    crossbeam::channel::Sender<T>,
    crossbeam::channel::Receiver<T>,
) {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            _ = size;
            crossbeam::channel::unbounded() // we're not allowed to block on web
        } else {
            crossbeam::channel::bounded(size)
        }
    }
}
