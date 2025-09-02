use std::sync::Arc;

use arrow::{
    array::{MapBuilder, StringBuilder},
    error::ArrowError,
};
use re_chunk::{Chunk, RowId, TimePoint};
use re_types::{
    ComponentBatch as _, ComponentDescriptor, SerializedComponentBatch, components,
    reflection::ComponentDescriptorExt as _,
};

use crate::Error;

use super::{Layer, LayerIdentifier};

/// Extracts a static summary of channel and schema information.
///
/// Can be used to get an overview over the contents of an MCAP file.
#[derive(Debug, Default)]
pub struct McapSchemaLayer;

impl Layer for McapSchemaLayer {
    fn identifier() -> LayerIdentifier {
        "schema".into()
    }

    fn process(
        &mut self,
        _mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error> {
        for channel in summary.channels.values() {
            let mut components = from_channel(channel)?;
            if let Some(schema) = channel.schema.as_ref() {
                components.extend(from_schema(schema)?);
            }

            let chunk = Chunk::builder(channel.topic.as_str())
                .with_archetype(RowId::new(), TimePoint::STATIC, &components)
                .build()?;
            emit(chunk);
        }

        Ok(())
    }
}

fn from_channel(
    channel: &Arc<::mcap::Channel<'_>>,
) -> Result<Vec<SerializedComponentBatch>, ArrowError> {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Channel {
        id,
        topic,
        schema: _, // handled by `fn from_schema` instead
        message_encoding,
        metadata,
    } = channel.as_ref();

    let key_builder = StringBuilder::new();
    let val_builder = StringBuilder::new();

    let mut builder = MapBuilder::new(None, key_builder, val_builder);

    for (key, val) in metadata {
        builder.keys().append_value(key);
        builder.values().append_value(val);
        builder.append(true)?;
    }

    let metadata = builder.finish();
    let archetype = "rerun.mcap.Channel";

    Ok(vec![
        SerializedComponentBatch::new(
            Arc::new(UInt16Array::from(vec![*id])),
            ComponentDescriptor::partial("id").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            Arc::new(StringArray::from(vec![topic.clone()])),
            ComponentDescriptor::partial("topic").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            Arc::new(metadata),
            ComponentDescriptor::partial("metadata").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            Arc::new(StringArray::from(vec![message_encoding.clone()])),
            ComponentDescriptor::partial("message_encoding").with_builtin_archetype(archetype),
        ),
    ])
}

fn from_schema(
    schema: &Arc<::mcap::Schema<'_>>,
) -> Result<Vec<SerializedComponentBatch>, re_types::SerializationError> {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Schema {
        id,
        name,
        encoding,
        data,
    } = schema.as_ref();

    let blob = components::Blob(data.clone().into_owned().into());

    // Adds a field of arbitrary data to this archetype.
    let archetype = "rerun.mcap.Schema";
    Ok(vec![
        SerializedComponentBatch::new(
            Arc::new(UInt16Array::from(vec![*id])),
            ComponentDescriptor::partial("id").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            Arc::new(StringArray::from(vec![name.clone()])),
            ComponentDescriptor::partial("name").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            blob.to_arrow()?,
            ComponentDescriptor::partial("data").with_builtin_archetype(archetype),
        ),
        SerializedComponentBatch::new(
            Arc::new(StringArray::from(vec![encoding.clone()])),
            ComponentDescriptor::partial("encoding").with_builtin_archetype(archetype),
        ),
    ])
}
