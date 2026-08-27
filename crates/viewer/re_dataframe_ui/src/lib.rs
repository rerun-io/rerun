//! Rich table widget over `datafusion`.

mod cards_view;
mod column_blueprint;
mod column_sorting;
mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod filters;
mod header_tooltip;
mod preview_renderer;
mod re_table;
mod re_table_utils;
mod streaming_cache;
mod table_blueprint;
mod table_blueprints;
mod table_selection;

pub use self::column_blueprint::{ColumnBlueprint, default_display_name_for_column};
pub use self::datafusion_table_widget::{DataFusionTableWidget, TableStatus};
pub use self::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
// for testing purposes
pub use self::column_sorting::{SortBy, SortDirection};
pub use self::filters::{
    ColumnFilter, ComparisonOperator, FloatFilter, IntFilter, NonNullableBooleanFilter,
    Nullability, NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
};
pub use self::header_tooltip::column_header_tooltip_ui;
pub use self::re_table_utils::{
    CELL_SEPARATOR_STROKE_OFFSET, apply_table_style_fixes, cell_ui, header_ui,
};
pub use self::streaming_cache::StreamingCacheTableProvider;
pub use self::table_blueprints::{TableBlueprintError, TableBlueprints};

/// Arrow field metadata keys for configuring card behavior.
///
/// These are read from [`arrow::datatypes::Field::metadata`] and populate the corresponding `TableBlueprint` fields.
pub mod experimental_field_metadata {
    /// Mark a boolean column as the flag/annotation toggle column.
    ///
    /// Set to `"true"` on a boolean field's metadata.
    pub const IS_FLAG_COLUMN: &str = "rerun:is_flag_column";

    /// Mark a column as the card title.
    ///
    /// Set to `"true"` on a field's metadata. If no column is marked, the first visible string column is used.
    pub const IS_GRID_VIEW_CARD_TITLE: &str = "rerun:is_grid_view_card_title";
}
