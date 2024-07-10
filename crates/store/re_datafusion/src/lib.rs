//! This is how we bind Rerun into datafusion
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

use datafusion::error::Result;
use datafusion::prelude::*;

use arrow::array::{Float64Array, Int32Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::sync::Arc;

pub fn create_datafusion_context() -> Result<SessionContext> {
    let ctx = SessionContext::new();

    // Define the schema for the in-memory table
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
    ]));

    // Create sample data
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["A", "B", "C"])),
            Arc::new(Float64Array::from(vec![10.0, 20.0, 30.0])),
        ],
    )?;

    // Register the in-memory table
    ctx.register_batch("my_table", batch)?;

    Ok(ctx)
}
