use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimePoint};
use re_sdk_types::archetypes::{McapChannel, McapSchema};
use re_sdk_types::{AsComponents as _, components};

use super::{Layer, LayerIdentifier};
use crate::Error;

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
            let mut components = from_channel(channel).as_serialized_batches();
            if let Some(schema) = channel.schema.as_ref() {
                components.extend(
                    McapSchema::update_fields()
                        .with_name(schema.name.clone())
                        .with_id(schema.id)
                        .with_encoding(schema.encoding.clone())
                        .with_data(schema.data.clone().into_owned())
                        .as_serialized_batches(),
                );
            }

            let chunk = Chunk::builder(channel.topic.as_str())
                .with_archetype(RowId::new(), TimePoint::STATIC, &components)
                .build()?;
            emit(chunk);
        }

        Ok(())
    }
}

fn from_channel(channel: &Arc<::mcap::Channel<'_>>) -> McapChannel {
    let ::mcap::Channel {
        id,
        topic,
        schema: _, // handled by `fn from_schema` instead
        message_encoding,
        metadata,
    } = channel.as_ref();

    let metadata_pairs: Vec<_> = metadata
        .iter()
        .map(|(key, val)| re_sdk_types::datatypes::Utf8Pair {
            first: key.clone().into(),
            second: val.clone().into(),
        })
        .collect();

    McapChannel::new(*id, topic.clone(), message_encoding.clone())
        .with_metadata(components::KeyValuePairs(metadata_pairs))
}
