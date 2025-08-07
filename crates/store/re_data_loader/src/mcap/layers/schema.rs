use std::sync::Arc;

use arrow::{
    array::{MapBuilder, StringBuilder},
    error::ArrowError,
};
use re_chunk::{Chunk, RowId, TimePoint};
use re_types::{AnyValues, components};

use crate::mcap::decode::PluginError;

use super::{LayerIdentifier, LayerNew};

/// Send static channel and schema information.
#[derive(Debug, Default)]
pub struct McapSchemaLayer;

impl LayerNew for McapSchemaLayer {
    fn identifier() -> LayerIdentifier {
        "schema".into()
    }

    fn process(
        &mut self,
        _mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), PluginError> {
        for channel in summary.channels.values() {
            let chunk = Chunk::builder(channel.topic.as_str())
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &[
                        from_channel(channel)?,
                        channel.schema.as_ref().map(from_schema).unwrap_or_default(),
                    ],
                )
                .build()?;
            emit(chunk);
        }

        Ok(())
    }
}

fn from_channel(channel: &Arc<::mcap::Channel<'_>>) -> Result<AnyValues, ArrowError> {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Channel {
        id,
        topic,
        schema: _, // Separate archetype
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

    Ok(AnyValues::new("rerun.mcap.Channel")
        .with_field("id", Arc::new(UInt16Array::from(vec![*id])))
        .with_field("topic", Arc::new(StringArray::from(vec![topic.clone()])))
        .with_field("metadata", Arc::new(metadata))
        .with_field(
            "message_encoding",
            Arc::new(StringArray::from(vec![message_encoding.clone()])),
        ))
}

fn from_schema(schema: &Arc<::mcap::Schema<'_>>) -> AnyValues {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Schema {
        id,
        name,
        encoding,
        data,
    } = schema.as_ref();

    let blob = components::Blob(data.clone().into_owned().into());

    // Adds a field of arbitrary data to this archetype.
    AnyValues::new("rerun.mcap.Schema")
        .with_field("id", Arc::new(UInt16Array::from(vec![*id])))
        .with_field("name", Arc::new(StringArray::from(vec![name.clone()])))
        .with_component::<components::Blob>("data", vec![blob])
        .with_field(
            "encoding",
            Arc::new(StringArray::from(vec![encoding.clone()])),
        )
}
