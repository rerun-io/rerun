use arrow::datatypes::{Schema as ArrowSchema, SchemaRef as ArrowSchemaRef};
use arrow::error::ArrowError;

/// Encode an arrow schema as IPC bytes.
pub fn ipc_from_schema(schema: &ArrowSchema) -> Result<Vec<u8>, ArrowError> {
    re_tracing::profile_function!();
    let mut ipc_bytes = Vec::<u8>::new();
    let mut writer = arrow::ipc::writer::StreamWriter::try_new(&mut ipc_bytes, schema)?;
    // writer.write(&batch)?;  // Is this needed?
    writer.finish()?;
    Ok(ipc_bytes)
}

/// Decode an arrow schema from IPC bytes, WITHOUT migration.
pub fn raw_schema_from_ipc(ipc_bytes: &[u8]) -> Result<ArrowSchemaRef, ArrowError> {
    re_tracing::profile_function!();
    let cursor = std::io::Cursor::new(ipc_bytes);
    let stream = arrow::ipc::reader::StreamReader::try_new(cursor, None)?;
    Ok(stream.schema())
}

/// Decode an arrow schema from IPC bytes, and migrate it to the latest sorbet version.
pub fn migrated_schema_from_ipc(ipc_bytes: &[u8]) -> Result<ArrowSchemaRef, ArrowError> {
    re_tracing::profile_function!();
    raw_schema_from_ipc(ipc_bytes).map(crate::migrate_schema_ref)
}
