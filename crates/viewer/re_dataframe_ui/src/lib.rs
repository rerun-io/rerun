//! Rich table widget over `datafusion`.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod convert_to_recording;
mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod header_tooltip;
mod requested_object;
mod table_blueprint;
pub mod table_utils;

pub use datafusion_table_widget::DataFusionTableWidget;
pub use display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
pub use header_tooltip::column_header_tooltip_ui;
pub use requested_object::RequestedObject;
pub use table_blueprint::{ColumnBlueprint, TableBlueprint, default_display_name_for_column};
