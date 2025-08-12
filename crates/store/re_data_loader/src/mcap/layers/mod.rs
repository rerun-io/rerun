mod protobuf;
mod raw;
mod recording_info;
mod ros2;
mod schema;
mod stats;

use re_chunk::{Chunk, EntityPath, external::nohash_hasher::IntMap};
use std::collections::{BTreeMap, BTreeSet};

pub use self::{
    protobuf::McapProtobufLayer, raw::McapRawLayer, recording_info::McapRecordingInfoLayer,
    ros2::McapRos2Layer, schema::McapSchemaLayer, stats::McapStatisticLayer,
};

use super::decode::{ChannelId, McapMessageParser, ParserContext, PluginError};

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
#[repr(transparent)]
pub struct LayerIdentifier(pub &'static str);

impl From<&'static str> for LayerIdentifier {
    fn from(value: &'static str) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for LayerIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait LayerNew {
    fn identifier() -> LayerIdentifier
    where
        Self: Sized;

    // TODO(grtlr): Consider abstracting over Summary
    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &::mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), PluginError>;
}

pub trait MessageLayer {
    fn identifier() -> LayerIdentifier
    where
        Self: Sized;

    fn init(&mut self, _summary: &::mcap::Summary) -> Result<(), PluginError> {
        Ok(())
    }

    /// Instantites a new parser that expects `num_rows` if it is interested in the current channel.
    ///
    /// Otherwise returns `None`.
    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn McapMessageParser>>;
}

impl<T: MessageLayer> LayerNew for T {
    fn identifier() -> LayerIdentifier {
        T::identifier()
    }

    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), PluginError> {
        re_tracing::profile_scope!("process-message-layer");
        self.init(summary)?;

        for chunk in &summary.chunk_indexes {
            re_tracing::profile_scope!("mcap-chunk");
            let channel_counts = super::util::get_chunk_message_count(chunk, summary, mcap_bytes)?;

            let parsers = summary
                .read_message_indexes(mcap_bytes, chunk)?
                .iter()
                .filter_map(|(channel, msg_offsets)| {
                    let entity_path = EntityPath::from(channel.topic.as_str());
                    Some((
                        ChannelId::from(channel.id),
                        (
                            ParserContext::new(entity_path),
                            self.message_parser(channel, msg_offsets.len())?,
                        ),
                    ))
                })
                .collect::<IntMap<_, _>>();

            re_log::trace!(
                "MCAP file contains {} channels with the following message counts: {:?}",
                channel_counts.len(),
                channel_counts
            );

            let mut decoder = super::decode::McapChunkDecoder::new(parsers);

            for msg in summary.stream_chunk(mcap_bytes, chunk)? {
                match msg {
                    Ok(message) => {
                        if let Err(err) = decoder.decode_next(&message) {
                            re_log::error!(
                                "Failed to decode message from MCAP file: {err} on channel: {}",
                                message.channel.topic
                            );
                        }
                    }
                    Err(err) => {
                        re_log::error!("Failed to read message from MCAP file: {err}");
                    }
                };
            }

            for chunk in decoder.finish() {
                if let Ok(chunk) = chunk {
                    emit(chunk);
                } else {
                    re_log::error!("Failed to decode chunk from MCAP file: {:?}", chunk);
                }
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct RegistryNew {
    factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn LayerNew>>,
}

impl RegistryNew {
    pub fn register<L: LayerNew + Default + 'static>(mut self) -> Self {
        if self
            .factories
            .insert(L::identifier(), || Box::new(L::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted layer {} twice.", L::identifier());
        };
        self
    }

    pub fn layers(
        &self,
        filter: Option<BTreeSet<String>>,
    ) -> impl Iterator<Item = Box<dyn LayerNew>> {
        re_log::debug!(
            "Existing layers: {:?}",
            self.factories.keys().collect::<Vec<_>>()
        );
        self.factories
            .iter()
            .filter_map(move |(identifier, factory)| {
                let Some(filter) = filter.as_ref() else {
                    return Some(factory());
                };

                if filter.contains(identifier.0) {
                    Some(factory())
                } else {
                    None
                }
            })
    }
}
