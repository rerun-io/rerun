mod metadata;
mod protobuf;
mod raw;
mod recording_info;
mod ros2;
mod ros2_reflection;
mod schema;
mod stats;

use std::collections::{BTreeMap, BTreeSet};

use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{Chunk, EntityPath};

pub use self::metadata::McapMetadataDecoder;
pub use self::protobuf::McapProtobufDecoder;
pub use self::raw::McapRawDecoder;
pub use self::recording_info::McapRecordingInfoDecoder;
pub use self::ros2::McapRos2Decoder;
pub use self::ros2_reflection::McapRos2ReflectionDecoder;
pub use self::schema::McapSchemaDecoder;
pub use self::stats::McapStatisticDecoder;
use crate::Error;
use crate::parsers::{ChannelId, MessageParser, ParserContext};

/// Globally unique identifier for a decoder.
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
#[repr(transparent)]
pub struct DecoderIdentifier(String);

impl From<&'static str> for DecoderIdentifier {
    fn from(value: &'static str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for DecoderIdentifier {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for DecoderIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A decoder describes information that can be extracted from an MCAP file.
///
/// It is the most general level at which we can interpret an MCAP file and can
/// be used to either output general information about the MCAP file or to call
/// into decoders that work on a per-message basis via the [`MessageDecoder`] trait.
pub trait Decoder {
    /// Globally unique identifier for this decoder.
    ///
    /// [`DecoderIdentifier`]s are also be used to select only a subset of active decoders.
    fn identifier() -> DecoderIdentifier
    where
        Self: Sized;

    /// The processing that needs to happen for this decoder.
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
/// This is a specialization of [`Decoder`] that allows defining [`MessageParser`]s.
/// to interpret the contents of MCAP chunks.
pub trait MessageDecoder {
    fn identifier() -> DecoderIdentifier
    where
        Self: Sized;

    fn init(&mut self, _summary: &::mcap::Summary) -> Result<(), Error> {
        Ok(())
    }

    /// Returns `true` if this decoder can handle the given channel.
    ///
    /// This method is used to determine which channels should be processed by which decoders,
    /// particularly for implementing fallback behavior where one decoder handles channels
    /// that other decoders cannot process.
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

        if let Some((ctx, parser)) = self.parsers.get_mut(&channel_id) {
            // If the parser fails, we should _not_ append the timepoint
            parser.append(ctx, msg)?;
            for timepoint in parser.get_log_and_publish_timepoints(msg)? {
                ctx.add_timepoint(timepoint);
            }
        } else {
            // TODO(#10862): If we encounter a message that we can't parse at all we should emit a warning.
            // Note that this quite easy to achieve when using decoders and only selecting a subset.
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

/// Used to select certain decoders.
#[derive(Clone, Debug)]
pub enum SelectedDecoders {
    All,
    Subset(BTreeSet<DecoderIdentifier>),
}

impl SelectedDecoders {
    /// Checks if a decoder is part of the current selection.
    pub fn contains(&self, value: &DecoderIdentifier) -> bool {
        match self {
            Self::All => true,
            Self::Subset(subset) => subset.contains(value),
        }
    }
}

/// Registry fallback strategy.
#[derive(Clone, Debug, Default)]
pub enum Fallback {
    /// No fallback – channels without a handler are simply unassigned.
    #[default]
    None,

    /// Single global fallback message decoder (e.g. `raw`).
    Global(DecoderIdentifier),
}

/// A runner that constrains a [`MessageDecoder`] to a specific set of channels.
pub struct MessageDecoderRunner {
    inner: Box<dyn MessageDecoder>,
    allowed: BTreeSet<ChannelId>,
}

impl MessageDecoderRunner {
    fn new(inner: Box<dyn MessageDecoder>, allowed: BTreeSet<ChannelId>) -> Self {
        Self { inner, allowed }
    }
}

impl Decoder for MessageDecoderRunner {
    fn identifier() -> DecoderIdentifier
    where
        Self: Sized,
    {
        // static identifier isn't used for trait objects; unreachable in practice.
        "message_decoder_runner".into()
    }

    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error> {
        self.inner.init(summary)?;

        for chunk in &summary.chunk_indexes {
            let parsers = summary
                .read_message_indexes(mcap_bytes, chunk)?
                .iter()
                .filter_map(|(channel, msg_offsets)| {
                    let channel_id = ChannelId::from(channel.id);
                    if !self.allowed.contains(&channel_id) {
                        return None;
                    }

                    let parser = self.inner.message_parser(channel, msg_offsets.len())?;
                    let entity_path = EntityPath::from(channel.topic.as_str());
                    let ctx = ParserContext::new(entity_path);
                    Some((channel_id, (ctx, parser)))
                })
                .collect::<IntMap<_, _>>();

            let mut decoder = McapChunkDecoder::new(parsers);

            for msg in summary.stream_chunk(mcap_bytes, chunk)? {
                match msg {
                    Ok(message) => {
                        if let Err(err) = decoder.decode_next(&message) {
                            re_log::error_once!(
                                "Failed to decode message on channel {}: {err}",
                                message.channel.topic
                            );
                        }
                    }
                    Err(err) => re_log::error!("Failed to read message from MCAP file: {err}"),
                }
            }

            for chunk in decoder.finish() {
                match chunk {
                    Ok(c) => emit(c),
                    Err(err) => re_log::error!("Failed to decode chunk: {err}"),
                }
            }
        }

        Ok(())
    }
}

/// A printable assignment used for dry-runs / UI.
#[derive(Clone, Debug)]
pub struct DecoderAssignment {
    pub channel_id: ChannelId,
    pub topic: String,
    pub encoding: String,
    pub schema_name: Option<String>,
    pub decoder: DecoderIdentifier,
}

/// A concrete execution plan for a given MCAP source.
pub struct ExecutionPlan {
    pub file_decoders: Vec<Box<dyn Decoder>>,
    pub runners: Vec<MessageDecoderRunner>,
    pub assignments: Vec<DecoderAssignment>,
}

impl ExecutionPlan {
    pub fn run(
        mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> anyhow::Result<()> {
        for mut decoder in self.file_decoders {
            decoder.process(mcap_bytes, summary, emit)?;
        }

        for runner in &mut self.runners {
            runner.process(mcap_bytes, summary, emit)?;
        }
        Ok(())
    }
}

/// Holds a set of all known decoders, split into file-scoped and message-scoped.
pub struct DecoderRegistry {
    file_factories: BTreeMap<DecoderIdentifier, fn() -> Box<dyn Decoder>>,
    msg_factories: BTreeMap<DecoderIdentifier, fn() -> Box<dyn MessageDecoder>>,
    msg_order: Vec<DecoderIdentifier>,
    fallback: Fallback,
}

impl DecoderRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self {
            file_factories: Default::default(),
            msg_factories: Default::default(),
            msg_order: Vec::new(),
            fallback: Fallback::None,
        }
    }

    /// Creates a registry with all builtin decoders and raw fallback enabled.
    pub fn all_with_raw_fallback() -> Self {
        Self::all_builtin(true)
    }

    /// Creates a registry with all builtin decoders and raw fallback disabled.
    pub fn all_without_raw_fallback() -> Self {
        Self::all_builtin(false)
    }

    /// Creates a registry with all builtin decoders with configurable raw fallback.
    pub fn all_builtin(raw_fallback_enabled: bool) -> Self {
        let mut registry = Self::empty()
            // file decoders:
            .register_file_decoder::<McapRecordingInfoDecoder>()
            .register_file_decoder::<McapMetadataDecoder>()
            .register_file_decoder::<McapSchemaDecoder>()
            .register_file_decoder::<McapStatisticDecoder>()
            // message decoders (priority order):
            .register_message_decoder::<McapRos2Decoder>()
            .register_message_decoder::<McapRos2ReflectionDecoder>()
            .register_message_decoder::<McapProtobufDecoder>();

        if raw_fallback_enabled {
            registry = registry
                .register_message_decoder::<McapRawDecoder>()
                .with_global_fallback::<McapRawDecoder>();
        } else {
            // still register raw so users can explicitly select it, just no fallback
            registry = registry.register_message_decoder::<McapRawDecoder>();
        }

        registry
    }

    /// Register a file-scoped decoder (runs once over the file/summary).
    pub fn register_file_decoder<L: Decoder + Default + 'static>(mut self) -> Self {
        let id = L::identifier();
        if self
            .file_factories
            .insert(id.clone(), || Box::new(L::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted file decoder {} twice.", id);
        }
        self
    }

    /// Register a message-scoped decoder (eligible to handle channels).
    pub fn register_message_decoder<M: MessageDecoder + Default + 'static>(mut self) -> Self {
        let id = <M as MessageDecoder>::identifier();
        if self
            .msg_factories
            .insert(id.clone(), || Box::new(M::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted message decoder {} twice.", id);
        }
        self.msg_order.push(id);
        self
    }

    /// Configure a global fallback message decoder (e.g. `raw`).
    pub fn with_global_fallback<M: MessageDecoder + 'static>(mut self) -> Self {
        self.fallback = Fallback::Global(<M as MessageDecoder>::identifier());
        self
    }

    /// Returns all registered decoder identifiers (file + message) as strings.
    pub fn all_identifiers(&self) -> Vec<String> {
        self.file_factories
            .keys()
            .chain(self.msg_factories.keys())
            .map(|id| id.to_string())
            .collect()
    }

    /// Produce a filtered registry that only contains `selected` decoders.
    pub fn select(&self, selected: &SelectedDecoders) -> Self {
        let file_factories = self
            .file_factories
            .iter()
            .filter(|(id, _)| selected.contains(id))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let msg_factories = self
            .msg_factories
            .iter()
            .filter(|(id, _)| selected.contains(id))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let msg_order = self
            .msg_order
            .iter()
            .filter(|&id| selected.contains(id))
            .cloned()
            .collect();

        let fallback = self.select_fallback(selected);

        Self {
            file_factories,
            msg_factories,
            msg_order,
            fallback,
        }
    }

    fn select_fallback(&self, selected: &SelectedDecoders) -> Fallback {
        match &self.fallback {
            Fallback::Global(id) if selected.contains(id) => Fallback::Global(id.clone()),
            Fallback::Global(_) | Fallback::None => Fallback::None,
        }
    }

    /// Build a concrete execution plan for a given file.
    pub fn plan(&self, summary: &mcap::Summary) -> anyhow::Result<ExecutionPlan> {
        let file_decoders = self
            .file_factories
            .values()
            .map(|f| f())
            .collect::<Vec<_>>();

        // instantiate message decoders and init them (supports_channel may depend on init)
        let mut msg_decoders: Vec<(DecoderIdentifier, Box<dyn MessageDecoder>)> = self
            .msg_order
            .iter()
            .filter_map(|id| self.msg_factories.get(id).map(|f| (id.clone(), f())))
            .collect();

        for (_, l) in &mut msg_decoders {
            l.init(summary)?;
        }

        let mut by_decoder: BTreeMap<DecoderIdentifier, BTreeSet<ChannelId>> = BTreeMap::new();
        let mut assignments: Vec<DecoderAssignment> = Vec::new();

        for channel_id in summary.channels.values() {
            // explicit priority order
            let mut chosen: Option<DecoderIdentifier> = None;
            for (id, decoder) in &msg_decoders {
                if decoder.supports_channel(channel_id.as_ref()) {
                    chosen = Some(id.clone());
                    break;
                }
            }

            if chosen.is_none() {
                // fallbacks (if any)
                if let Fallback::Global(id) = &self.fallback
                    && self.msg_factories.contains_key(id)
                {
                    chosen = Some(id.clone());
                }
            }

            let schema_name = channel_id.schema.as_ref().map(|s| s.name.clone());

            let schema_encoding = channel_id
                .schema
                .as_ref()
                .map(|s| s.encoding.as_str())
                .unwrap_or("Unknown");

            if let Some(id) = chosen {
                by_decoder
                    .entry(id.clone())
                    .or_default()
                    .insert(ChannelId::from(channel_id.id));

                assignments.push(DecoderAssignment {
                    channel_id: ChannelId::from(channel_id.id),
                    topic: channel_id.topic.clone(),
                    encoding: schema_encoding.to_owned(),
                    schema_name: channel_id.schema.as_ref().map(|s| s.name.clone()),
                    decoder: id,
                });
            } else {
                re_log::debug!(
                    "No message decoder selected for topic '{}' (encoding='{}', schema='{:?}')",
                    channel_id.topic,
                    schema_encoding,
                    schema_name,
                );
            }
        }

        let mut runners = Vec::new();
        for (decoder_id, allowed) in by_decoder {
            if let Some(factory) = self.msg_factories.get(&decoder_id) {
                let inner = factory();
                runners.push(MessageDecoderRunner::new(inner, allowed));
            }
        }

        Ok(ExecutionPlan {
            file_decoders,
            runners,
            assignments,
        })
    }
}
