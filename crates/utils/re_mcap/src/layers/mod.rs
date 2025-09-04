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

use crate::{
    Error,
    parsers::{ChannelId, MessageParser, ParserContext},
};

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
/// be used to either output general information about the MCAP file or to call
/// into layers that work on a per-message basis via the [`MessageLayer`] trait.
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
    ) -> Result<(), Error>;
}

/// Can be used to extract per-message information from an MCAP file.
///
/// This is a specialization of [`Layer`] that allows defining [`MessageParser`]s.
/// to interpret the contents of MCAP chunks.
pub trait MessageLayer {
    fn identifier() -> LayerIdentifier
    where
        Self: Sized;

    fn init(&mut self, _summary: &::mcap::Summary) -> Result<(), Error> {
        Ok(())
    }

    /// Returns `true` if this layer can handle the given channel.
    ///
    /// This method is used to determine which channels should be processed by which layers,
    /// particularly for implementing fallback behavior where one layer handles channels
    /// that other layers cannot process.
    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool;

    /// Instantites a new [`MessageParser`] that expects `num_rows` if it is interested in the current channel.
    ///
    /// Otherwise returns `None`.
    ///
    /// The `num_rows` argument allows parsers to pre-allocate storage with the
    /// correct capacity, avoiding reallocations during message processing.
    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>>;
}

type Parser = (ParserContext, Box<dyn MessageParser>);

/// Decodes batches of messages from an MCAP into Rerun chunks using previously registered parsers.
struct McapChunkDecoder {
    parsers: IntMap<ChannelId, Parser>,
}

impl McapChunkDecoder {
    pub fn new(parsers: IntMap<ChannelId, Parser>) -> Self {
        Self { parsers }
    }

    /// Decode the next message in the chunk
    pub fn decode_next(&mut self, msg: &::mcap::Message<'_>) -> Result<(), Error> {
        re_tracing::profile_function!();

        let channel = msg.channel.as_ref();
        let channel_id = ChannelId(channel.id);
        let timepoint = re_chunk::TimePoint::from([
            (
                "log_time",
                re_log_types::TimeCell::from_timestamp_nanos_since_epoch(msg.log_time as i64),
            ),
            (
                "publish_time",
                re_log_types::TimeCell::from_timestamp_nanos_since_epoch(msg.publish_time as i64),
            ),
        ]);

        if let Some((ctx, parser)) = self.parsers.get_mut(&channel_id) {
            // If the parser fails, we should _not_ append the timepoint
            parser.append(ctx, msg)?;
            ctx.add_timepoint(timepoint.clone());
        } else {
            // TODO(#10867): If we encounter a message that we can't parse at all we should emit a warning.
            // Note that this quite easy to achieve when using layers and only selecting a subset.
            // However, to not overwhelm the user this should be reported in a _single_ static chunk,
            // so this is not the right place for this. Maybe we need to introduce something like a "report".
        }
        Ok(())
    }

    /// Finish the decoding process and return the chunks.
    pub fn finish(self) -> impl Iterator<Item = Result<Chunk, Error>> {
        self.parsers
            .into_values()
            .flat_map(|(ctx, parser)| match parser.finalize(ctx) {
                Ok(chunks) => chunks.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(err) => vec![Err(Error::Other(err))],
            })
    }
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
    ) -> Result<(), Error> {
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

            let mut decoder = McapChunkDecoder::new(parsers);

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
pub struct LayerRegistry {
    factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn Layer>>,
}

impl LayerRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self {
            factories: Default::default(),
        }
    }

    /// Creates a registry with all builtin layers.
    pub fn all() -> Self {
        Self::all_with_raw_fallback(true)
    }

    /// Creates a registry with all builtin layers with configurable raw fallback.
    pub fn all_with_raw_fallback(raw_fallback_enabled: bool) -> Self {
        let mut registry = Self::empty()
            .register::<McapProtobufLayer>()
            .register::<McapRecordingInfoLayer>()
            .register::<McapRos2Layer>()
            .register::<McapSchemaLayer>()
            .register::<McapStatisticLayer>();

        // Add raw layer based on fallback setting
        if raw_fallback_enabled {
            registry = registry
                .register_with_factory(<McapRawLayer as Layer>::identifier(), || {
                    Box::new(McapRawLayer::default().with_fallback_enabled(true)) as Box<dyn Layer>
                });
        } else {
            registry = registry
                .register_with_factory(<McapRawLayer as Layer>::identifier(), || {
                    Box::new(McapRawLayer::default().with_fallback_enabled(false)) as Box<dyn Layer>
                });
        }

        registry
    }

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

    /// Adds a layer to the registry with a custom factory function.
    pub fn register_with_factory<S: Into<LayerIdentifier>>(
        mut self,
        identifier: S,
        factory: fn() -> Box<dyn Layer>,
    ) -> Self {
        let identifier = identifier.into();
        if self.factories.insert(identifier.clone(), factory).is_some() {
            re_log::warn_once!("Inserted layer {} twice.", identifier);
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
