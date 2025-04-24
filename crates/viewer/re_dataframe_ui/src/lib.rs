//! Rich table widget over `datafusion`.

mod datafusion_adapter;
mod display_record_batch;
mod requested_object;
mod table_ui;
pub mod table_utils;

pub use display_record_batch::{DisplayColumn, DisplayRecordBatch, DisplayRecordBatchError};
pub use requested_object::RequestedObject;
pub use table_ui::DataFusionTableWidget;
