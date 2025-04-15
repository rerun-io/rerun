//! Rich table widget over `datafusion`.

mod display_record_batch;
mod requested_object;
pub mod table_utils;

pub use display_record_batch::{DisplayColumn, DisplayRecordBatch, DisplayRecordBatchError};
pub use requested_object::RequestedObject;
