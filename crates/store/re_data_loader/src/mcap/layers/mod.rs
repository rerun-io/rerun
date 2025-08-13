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

/// Globally unique identifier for a layer.
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
#[repr(transparent)]
pub struct LayerIdentifier(String);

impl From<&'static str> for LayerIdentifier {
    fn from(value: &'static str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for LayerIdentifier {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for LayerIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A layer describes information that can be extracted from an MCAP file.
///
/// It is the most general level at which we can interpret an MCAP file and can
/// be used to either output general information about the MCAP file (e.g.
/// [`McapStatisticLayer`]) or to call into layers that work on a per-message
/// basis via the [`MessageLayer`] trait.
pub trait Layer {
    /// Globally unique identifier for this layer.
    ///
    /// [`LayerIdentifier`]s are also be used to select only a subset of active layers.
    fn identifier() -> LayerIdentifier
    where
        Self: Sized;

    /// The processing that needs to happen for this layer.
    ///
    /// This function has access to the entire MCAP file via `mcap_bytes`.
    // TODO(#10862): Consider abstracting over `Summary` to allow more convenient / performant indexing.
    // For example, we probably don't want to store the entire file in memory.
    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &::mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), PluginError>;
}

/// Can be used to extract per-message information from an MCAP file.
///
/// This is a specialization of [`Layer`] that allows defining [`McapMessageParsers`]
/// to interpret the contents of MCAP chunks.
pub trait MessageLayer {
    fn identifier() -> LayerIdentifier
    where
        Self: Sized;

    fn init(&mut self, _summary: &::mcap::Summary) -> Result<(), PluginError> {
        Ok(())
    }

    /// Instantites a new [`McapMessageParser`] that expects `num_rows` if it is interested in the current channel.
    ///
    /// Otherwise returns `None`.
    ///
    /// The `num_rows` argument allows parsers to pre-allocate storage with the
    /// correct capacity, avoiding reallocations during message processing.
    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn McapMessageParser>>;
}

impl<T: MessageLayer> Layer for T {
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
                    let parser = self.message_parser(channel, msg_offsets.len())?;
                    let entity_path = EntityPath::from(channel.topic.as_str());
                    let ctx = ParserContext::new(entity_path);
                    Some((ChannelId::from(channel.id), (ctx, parser)))
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
                }
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

/// Used to select certain layers.
#[derive(Clone, Debug)]
pub enum SelectedLayers {
    All,
    Subset(BTreeSet<LayerIdentifier>),
}

impl SelectedLayers {
    /// Checks if a layer is part of the current selection.
    pub fn contains(&self, value: &LayerIdentifier) -> bool {
        match self {
            Self::All => true,
            Self::Subset(subset) => subset.contains(value),
        }
    }
}

/// Holds a set of all known layers.
///
/// Custom layers can be added by implementing the [`Layer`] or [`MessageLayer`]
/// traits and calling [`Self::register`].
#[derive(Default)]
pub struct LayerRegistry {
    factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn Layer>>,
}

impl LayerRegistry {
    /// Adds an additional layer to the registry.
    pub fn register<L: Layer + Default + 'static>(mut self) -> Self {
        if self
            .factories
            .insert(L::identifier(), || Box::new(L::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted layer {} twice.", L::identifier());
        }
        self
    }

    /// Returns a list of all layers.
    pub fn layers(&self, selected: SelectedLayers) -> impl Iterator<Item = Box<dyn Layer>> {
        re_log::debug!(
            "Existing layers: {:?}",
            self.factories.keys().collect::<Vec<_>>()
        );
        self.factories
            .iter()
            .filter_map(move |(identifier, factory)| {
                let SelectedLayers::Subset(selected) = &selected else {
                    return Some(factory());
                };

                if selected.contains(identifier) {
                    Some(factory())
                } else {
                    None
                }
            })
    }
}
