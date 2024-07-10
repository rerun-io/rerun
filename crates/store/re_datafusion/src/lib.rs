//! This is how we bind Rerun into datafusion
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod chunk_table;

use chunk_table::CustomDataSource;
use datafusion::error::Result;
use datafusion::prelude::*;

use std::sync::Arc;

pub fn create_datafusion_context() -> Result<SessionContext> {
    let ctx = SessionContext::new();

    let db = CustomDataSource::default();

    ctx.register_table("custom_table", Arc::new(db))?;

    Ok(ctx)
}
