//! This is how we bind Rerun into datafusion
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod chunk_table;
mod field_extraction;

use chunk_table::CustomDataSource;
use datafusion::error::Result;
use datafusion::logical_expr::ScalarUDF;
use datafusion::prelude::*;
use field_extraction::ExtractField;
use re_chunk_store::ChunkStore;

use std::sync::Arc;

pub fn create_datafusion_context(store: ChunkStore) -> Result<SessionContext> {
    let extract_field = ScalarUDF::from(ExtractField::new());

    let ctx = SessionContext::new();

    ctx.register_udf(extract_field.clone());

    let db = CustomDataSource::new(store);

    ctx.register_table("custom_table", Arc::new(db))?;

    Ok(ctx)
}
