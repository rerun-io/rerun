use arrow::{
    datatypes::{Schema as ArrowSchema, SchemaRef as ArrowSchemaRef},
    error::ArrowError,
};

/// Encode an arrow schema as IPC bytes.
pub fn ipc_from_schema(schema: &ArrowSchema) -> Result<Vec<u8>, ArrowError> {
    let mut ipc_bytes = Vec::<u8>::new();
    let mut writer = arrow::ipc::writer::StreamWriter::try_new(&mut ipc_bytes, schema)?;
    // writer.write(&batch)?;  // Is this needed?
    writer.finish()?;
    Ok(ipc_bytes)
}

/// Decode an arrow schema from IPC bytes.
pub fn schema_from_ipc(ipc_bytes: &[u8]) -> Result<ArrowSchemaRef, ArrowError> {
    let cursor = std::io::Cursor::new(ipc_bytes);
    let stream = arrow::ipc::reader::StreamReader::try_new(cursor, None)?;
    Ok(stream.schema())
}
