//! Rich table widget over `datafusion`.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod filters;
mod header_tooltip;
mod requested_object;
mod table_blueprint;
pub mod table_utils;

pub use self::{
    datafusion_table_widget::{DataFusionTableWidget, TableStatus},
    display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError},
    header_tooltip::column_header_tooltip_ui,
    requested_object::RequestedObject,
    table_blueprint::{
        ColumnBlueprint, SortBy, SortDirection, TableBlueprint, default_display_name_for_column,
    },
};

// for testing purposes
pub use self::filters::{
    ColumnFilter, ComparisonOperator, Filter, FloatFilter, IntFilter, NonNullableBooleanFilter,
    Nullability, NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
};
