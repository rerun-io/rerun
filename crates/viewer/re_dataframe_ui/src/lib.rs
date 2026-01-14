//! Rich table widget over `datafusion`.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod filters;
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
pub use self::streaming_cache::StreamingCacheTableProvider;
pub use self::table_blueprint::{
    ColumnBlueprint, SortBy, SortDirection, TableBlueprint, default_display_name_for_column,
};
