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
use re_log_types::TimeType;

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
use crate::util::collect_empty_channels;

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
    /// This function has access to all file-level processing inputs via [`DecoderContext`].
    // TODO(#10862): Consider abstracting over `Summary` to allow more convenient / performant indexing.
    // For example, we probably don't want to store the entire file in memory.
    fn process(
        &mut self,
        ctx: &DecoderContext<'_>,
        emit: &mut dyn FnMut(Chunk),
    ) -> Result<(), Error>;
}

/// Shared processing context of a decode run.
pub struct DecoderContext<'a> {
    mcap_bytes: &'a [u8],
    summary: &'a ::mcap::Summary,
    topic_filter: &'a TopicFilter,
    empty_channels: BTreeSet<ChannelId>,
}

impl<'a> DecoderContext<'a> {
    pub fn new(
        mcap_bytes: &'a [u8],
        summary: &'a ::mcap::Summary,
        topic_filter: &'a TopicFilter,
        empty_channels: BTreeSet<ChannelId>,
    ) -> Self {
        Self {
            mcap_bytes,
            summary,
            topic_filter,
            empty_channels,
        }
    }

    pub fn summary(&self) -> &'a ::mcap::Summary {
        self.summary
    }

    /// Returns an iterator over all MCAP channels that are relevant for processing
    /// in this context. I.e. all channels after removing empties, applying filters etc.
    pub fn relevant_channels(&self) -> impl Iterator<Item = &std::sync::Arc<mcap::Channel<'a>>> {
        self.summary.channels.values().filter(|channel| {
            !self.empty_channels.contains(&ChannelId(channel.id))
                && self.topic_filter.matches(&channel.topic)
        })
    }

    /// Iterates metadata records referenced by the summary metadata index.
    pub fn metadata_records(
        &self,
    ) -> impl Iterator<
        Item = (
            &'a mcap::records::MetadataIndex,
            Result<mcap::records::Metadata, mcap::McapError>,
        ),
    > + '_ {
        self.summary
            .metadata_indexes
            .iter()
            .map(|index| (index, mcap::read::metadata(self.mcap_bytes, index)))
    }
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
    time_type: TimeType,
}

impl McapChunkDecoder {
    pub fn new(parsers: IntMap<ChannelId, Parser>, time_type: TimeType) -> Self {
        Self { parsers, time_type }
    }

    /// Decode the next message in the chunk
    pub fn decode_next(&mut self, msg: &::mcap::Message<'_>) -> Result<(), Error> {
        re_tracing::profile_function!();

        let channel = msg.channel.as_ref();
        let channel_id = ChannelId(channel.id);

        if let Some((ctx, parser)) = self.parsers.get_mut(&channel_id) {
            // If the parser fails, we should _not_ append the timepoint
            parser.append(ctx, msg)?;
            for timepoint in parser.get_log_and_publish_timepoints(msg, self.time_type)? {
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

/// Regex-based filter selecting which MCAP topics to decode.
///
/// Patterns use [RE2 syntax](https://github.com/google/re2/wiki/Syntax).
///
/// A topic is kept if:
/// - `include` is empty, **or** any pattern in `include` matches; **and**
/// - no pattern in `exclude` matches.
///
/// Patterns are not implicitly anchored; use `^` / `$` if you need anchoring.
#[derive(Default, Clone, Debug)]
pub struct TopicFilter {
    include: Vec<regex_lite::Regex>,
    exclude: Vec<regex_lite::Regex>,
}

impl TopicFilter {
    pub fn with_include_patterns(mut self, include: &[String]) -> Result<Self, regex_lite::Error> {
        self.include = include
            .iter()
            .map(|pattern| regex_lite::Regex::new(pattern))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    pub fn with_exclude_patterns(mut self, exclude: &[String]) -> Result<Self, regex_lite::Error> {
        self.exclude = exclude
            .iter()
            .map(|pattern| regex_lite::Regex::new(pattern))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Returns `true` if the given topic passes the filter.
    pub fn matches(&self, topic: &str) -> bool {
        let included = self.include.is_empty() || self.include.iter().any(|r| r.is_match(topic));
        let excluded = self.exclude.iter().any(|r| r.is_match(topic));
        included && !excluded
    }

    /// Returns `true` if no patterns are configured (i.e. all topics pass).
    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
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

    fn process(
        &mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        time_type: TimeType,
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
                    let ctx = ParserContext::new(entity_path, channel.topic.clone(), time_type);
                    Some((channel_id, (ctx, parser)))
                })
                .collect::<IntMap<_, _>>();

            let mut decoder = McapChunkDecoder::new(parsers, time_type);

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

            for mut chunk in decoder.finish() {
                if let Ok(chunk) = &mut chunk {
                    chunk.sort_if_unsorted();
                    for (name, column) in chunk.timelines() {
                        if !column.is_sorted() {
                            let entity_path = chunk.entity_path();
                            re_log::warn_once!(
                                "Found unsorted timeline '{name}' for entity '{entity_path}'. This may lead to suboptimal performance.",
                            );
                        }
                    }
                }

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
    pub topic_filter: TopicFilter,
}

impl ExecutionPlan {
    pub fn run(
        mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        time_type: TimeType,
        emit: &mut dyn FnMut(Chunk),
    ) -> anyhow::Result<()> {
        let empty_channels = collect_empty_channels(mcap_bytes, summary)?;
        let ctx = DecoderContext::new(mcap_bytes, summary, &self.topic_filter, empty_channels);

        for mut decoder in self.file_decoders {
            decoder.process(&ctx, emit)?;
        }

        for runner in &mut self.runners {
            runner.process(mcap_bytes, summary, time_type, emit)?;
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
    pub fn plan(
        &self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        topic_filter: &TopicFilter,
    ) -> anyhow::Result<ExecutionPlan> {
        let file_decoders = self
            .file_factories
            .values()
            .map(|f| f())
            .collect::<Vec<_>>();

        let empty_channels = collect_empty_channels(mcap_bytes, summary)?;

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
            let channel_id = ChannelId::from(channel_id.id);
            let channel = summary.channels[&channel_id.0].as_ref();

            if empty_channels.contains(&channel_id) {
                re_log::debug!(
                    "Skipping MCAP channel '{}' (id={}) because it contains no messages.",
                    channel.topic,
                    channel_id.0,
                );
                continue;
            }

            if channel.message_encoding.trim().is_empty() {
                re_log::warn_once!(
                    "MCAP channel '{}' does not specify a message encoding.",
                    channel.topic,
                );
            }

            if !topic_filter.matches(&channel.topic) {
                re_log::debug!(
                    "Skipping MCAP channel '{}' because it does not match the topic filter.",
                    channel.topic,
                );
                continue;
            }

            // explicit priority order
            let mut chosen: Option<DecoderIdentifier> = None;
            for (id, decoder) in &msg_decoders {
                if decoder.supports_channel(channel) {
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

            let schema_name = channel.schema.as_ref().map(|s| s.name.clone());

            let schema_encoding = channel
                .schema
                .as_ref()
                .map(|s| s.encoding.as_str())
                .unwrap_or("Unknown");

            if let Some(id) = chosen {
                by_decoder.entry(id.clone()).or_default().insert(channel_id);

                assignments.push(DecoderAssignment {
                    channel_id,
                    topic: channel.topic.clone(),
                    encoding: schema_encoding.to_owned(),
                    schema_name: channel.schema.as_ref().map(|s| s.name.clone()),
                    decoder: id,
                });
            } else {
                re_log::debug!(
                    "No message decoder selected for topic '{}' (encoding='{}', schema='{:?}')",
                    channel.topic,
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
            topic_filter: topic_filter.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use re_chunk::Chunk;
    use re_log_types::TimeType;
    use re_sdk_types::archetypes::McapMessage;

    use super::*;

    #[test]
    fn skips_channels_without_messages() {
        let (summary, buffer, empty_channel_id, active_channel_id) = {
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let empty_channel_id = writer
                .add_channel(0, "empty_topic", "raw", &Default::default())
                .expect("failed to add empty channel");
            let active_channel_id = writer
                .add_channel(0, "active_topic", "raw", &Default::default())
                .expect("failed to add active channel");

            writer
                .write_to_known_channel(
                    &mcap::records::MessageHeader {
                        channel_id: active_channel_id,
                        sequence: 0,
                        log_time: 1,
                        publish_time: 1,
                    },
                    &[1, 2, 3],
                )
                .expect("failed to write message");

            let summary = writer.finish().expect("failed to finish writer");
            let buffer = writer.into_inner().into_inner();

            (summary, buffer, empty_channel_id, active_channel_id)
        };

        let plan = DecoderRegistry::empty()
            .register_file_decoder::<McapSchemaDecoder>()
            .register_message_decoder::<McapRawDecoder>()
            .plan(&buffer, &summary, &TopicFilter::default())
            .expect("failed to plan");

        assert_eq!(plan.assignments.len(), 1);
        assert_eq!(plan.assignments[0].channel_id, ChannelId(active_channel_id));
        assert_ne!(plan.assignments[0].channel_id, ChannelId(empty_channel_id));

        let mut chunks = Vec::<Chunk>::new();
        plan.run(&buffer, &summary, TimeType::TimestampNs, &mut |chunk| {
            chunks.push(chunk);
        })
        .expect("failed to run plan");

        assert_eq!(chunks.len(), 2);
        assert!(
            chunks
                .iter()
                .all(|chunk| !chunk.entity_path().to_string().ends_with("empty_topic"))
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.entity_path().to_string().ends_with("active_topic"))
        );
    }

    /// Test helper for creating an MCAP summary & blob with a ros2msg-schema channel.
    fn ros2_summary_with_message_encoding(
        schema_name: &str,
        topic: &str,
        message_encoding: &str,
        payload: &[u8],
    ) -> (mcap::Summary, Vec<u8>) {
        let cursor = io::Cursor::new(Vec::new());
        let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");
        let schema_id = writer
            .add_schema(schema_name, "ros2msg", b"string data")
            .expect("failed to add schema");
        let channel_id = writer
            .add_channel(schema_id, topic, message_encoding, &Default::default())
            .expect("failed to add channel");

        writer
            .write_to_known_channel(
                &mcap::records::MessageHeader {
                    channel_id,
                    sequence: 0,
                    log_time: 1,
                    publish_time: 1,
                },
                payload,
            )
            .expect("failed to write message");

        let summary = writer.finish().expect("failed to finish writer");
        let buffer = writer.into_inner().into_inner();
        (summary, buffer)
    }

    /// We expect CDR as encoding for ros2msg-schema messages.
    /// Test that a non-CDR channel that claims to have ros2msg
    /// falls back to raw forwarding instead of message reflection.
    #[test]
    fn non_cdr_ros2msg_channel_is_forwarded_as_raw_blob() {
        let (summary, buffer) = ros2_summary_with_message_encoding(
            "custom_msgs/msg/Foo",
            "non_cdr_topic",
            "json",
            br#"{"data":"hello"}"#,
        );

        let plan = DecoderRegistry::all_with_raw_fallback()
            .plan(&buffer, &summary, &TopicFilter::default())
            .expect("failed to plan");

        let assignment = plan
            .assignments
            .iter()
            .find(|assignment| assignment.topic == "non_cdr_topic")
            .expect("missing assignment");
        assert_eq!(assignment.decoder.to_string(), "raw");

        let mut chunks = Vec::<Chunk>::new();
        plan.run(&buffer, &summary, TimeType::TimestampNs, &mut |chunk| {
            chunks.push(chunk);
        })
        .expect("failed to run plan");

        assert!(chunks.iter().any(|chunk| {
            chunk.entity_path().to_string().ends_with("non_cdr_topic")
                && chunk
                    .component_descriptors()
                    .any(|descr| descr.component == McapMessage::descriptor_data().component)
        }));
    }

    /// Tests that semantic ROS 2 parsers also reject non-CDR channels.
    #[test]
    fn semantic_ros2_decoder_does_not_claim_non_cdr_channels() {
        let (summary, buffer) = ros2_summary_with_message_encoding(
            "std_msgs/msg/String",
            "non_cdr_string_topic",
            "json",
            br#"{"data":"hello"}"#,
        );

        let plan = DecoderRegistry::all_with_raw_fallback()
            .plan(&buffer, &summary, &TopicFilter::default())
            .expect("failed to plan");

        let assignment = plan
            .assignments
            .iter()
            .find(|assignment| assignment.topic == "non_cdr_string_topic")
            .expect("missing assignment");
        assert_eq!(assignment.decoder.to_string(), "raw");
    }

    #[test]
    fn topic_filter_matches() {
        // Empty filter accepts everything.
        let filter = TopicFilter::default();
        assert!(filter.is_empty());
        assert!(filter.matches("/anything"));
        assert!(filter.matches("/foo/bar"));

        // Pure include: only matching topics pass.
        let filter = TopicFilter {
            include: vec![regex_lite::Regex::new(r"^/camera/").unwrap()],
            exclude: vec![],
        };
        assert!(!filter.is_empty());
        assert!(filter.matches("/camera/rgb"));
        assert!(filter.matches("/camera/depth"));
        assert!(!filter.matches("/imu"));

        // Pure exclude: empty include means everything passes except excluded.
        let filter = TopicFilter {
            include: vec![],
            exclude: vec![regex_lite::Regex::new(r"^/diagnostics").unwrap()],
        };
        assert!(filter.matches("/camera/rgb"));
        assert!(!filter.matches("/diagnostics/agg"));

        // Combined: exclude takes precedence over include.
        let filter = TopicFilter {
            include: vec![regex_lite::Regex::new(r"^/camera/").unwrap()],
            exclude: vec![regex_lite::Regex::new(r"depth$").unwrap()],
        };
        assert!(filter.matches("/camera/rgb"));
        assert!(!filter.matches("/camera/depth"));
        assert!(!filter.matches("/imu"));

        // Multiple includes: match if ANY matches.
        let filter = TopicFilter {
            include: vec![
                regex_lite::Regex::new(r"^/camera/").unwrap(),
                regex_lite::Regex::new(r"^/imu$").unwrap(),
            ],
            exclude: vec![],
        };
        assert!(filter.matches("/camera/rgb"));
        assert!(filter.matches("/imu"));
        assert!(!filter.matches("/lidar"));
    }

    #[test]
    fn filter_skips_unselected_topics() {
        let (summary, buffer) = {
            let cursor = io::Cursor::new(Vec::new());
            let mut writer = mcap::Writer::new(cursor).expect("failed to create writer");

            let camera_rgb = writer
                .add_channel(0, "/camera/rgb", "raw", &Default::default())
                .expect("failed to add channel");
            let camera_depth = writer
                .add_channel(0, "/camera/depth", "raw", &Default::default())
                .expect("failed to add channel");
            let imu = writer
                .add_channel(0, "/imu", "raw", &Default::default())
                .expect("failed to add channel");

            for channel_id in [camera_rgb, camera_depth, imu] {
                writer
                    .write_to_known_channel(
                        &mcap::records::MessageHeader {
                            channel_id,
                            sequence: 0,
                            log_time: 1,
                            publish_time: 1,
                        },
                        &[1, 2, 3],
                    )
                    .expect("failed to write message");
            }

            let summary = writer.finish().expect("failed to finish writer");
            let buffer = writer.into_inner().into_inner();
            (summary, buffer)
        };

        // Include only /camera/* topics.
        let filter = TopicFilter {
            include: vec![regex_lite::Regex::new(r"^/camera/").unwrap()],
            exclude: vec![],
        };

        let plan = DecoderRegistry::empty()
            .register_message_decoder::<McapRawDecoder>()
            .plan(&buffer, &summary, &filter)
            .expect("failed to plan");

        assert_eq!(plan.assignments.len(), 2);
        let topics: BTreeSet<_> = plan.assignments.iter().map(|a| a.topic.as_str()).collect();
        assert!(topics.contains("/camera/rgb"));
        assert!(topics.contains("/camera/depth"));
        assert!(!topics.contains("/imu"));

        // Exclude /camera/depth.
        let filter = TopicFilter {
            include: vec![],
            exclude: vec![regex_lite::Regex::new(r"depth$").unwrap()],
        };

        let plan = DecoderRegistry::empty()
            .register_message_decoder::<McapRawDecoder>()
            .plan(&buffer, &summary, &filter)
            .expect("failed to plan");

        let topics: BTreeSet<_> = plan.assignments.iter().map(|a| a.topic.as_str()).collect();
        assert_eq!(topics.len(), 2);
        assert!(topics.contains("/camera/rgb"));
        assert!(topics.contains("/imu"));
        assert!(!topics.contains("/camera/depth"));

        // Include + exclude combined.
        let filter = TopicFilter {
            include: vec![regex_lite::Regex::new(r"^/camera/").unwrap()],
            exclude: vec![regex_lite::Regex::new(r"depth$").unwrap()],
        };

        let plan = DecoderRegistry::empty()
            .register_message_decoder::<McapRawDecoder>()
            .plan(&buffer, &summary, &filter)
            .expect("failed to plan");

        assert_eq!(plan.assignments.len(), 1);
        assert_eq!(plan.assignments[0].topic, "/camera/rgb");
    }
}
