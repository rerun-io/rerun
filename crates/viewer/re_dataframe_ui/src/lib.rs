//! Rich table widget over `datafusion`.

mod datafusion_adapter;
mod datafusion_table_widget;
mod display_record_batch;
mod requested_object;
mod table_blueprint;
pub mod table_utils;

pub use datafusion_table_widget::DataFusionTableWidget;
pub use display_record_batch::{DisplayColumn, DisplayRecordBatch, DisplayRecordBatchError};
pub use requested_object::RequestedObject;
