//! Rich table widget over `datafusion`.

mod blueprint;
mod cards_view;
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
mod table_blueprints;
mod table_selection;

pub use self::blueprint::TableColumn;
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
pub use re_sdk_types::blueprint::components::TableCellKind;
