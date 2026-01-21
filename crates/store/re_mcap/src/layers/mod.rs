mod foxglove;
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
use re_lenses::Lenses;

pub use self::foxglove::foxglove_lenses;
pub use self::protobuf::McapProtobufLayer;
pub use self::raw::McapRawLayer;
pub use self::recording_info::McapRecordingInfoLayer;
pub use self::ros2::McapRos2Layer;
pub use self::ros2_reflection::McapRos2ReflectionLayer;
pub use self::schema::McapSchemaLayer;
pub use self::stats::McapStatisticLayer;
use crate::Error;
use crate::parsers::{ChannelId, MessageParser, ParserContext};

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

        if let Some((ctx, parser)) = self.parsers.get_mut(&channel_id) {
            // If the parser fails, we should _not_ append the timepoint
            parser.append(ctx, msg)?;
            for timepoint in parser.get_log_and_publish_timepoints(msg)? {
                ctx.add_timepoint(timepoint);
            }
        } else {
            // TODO(#10862): If we encounter a message that we can't parse at all we should emit a warning.
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

/// Registry fallback strategy.
#[derive(Clone, Debug, Default)]
pub enum Fallback {
    /// No fallback – channels without a handler are simply unassigned.
    #[default]
    None,

    /// Single global fallback message layer (e.g. `raw`).
    Global(LayerIdentifier),
}

/// A runner that constrains a [`MessageLayer`] to a specific set of channels.
pub struct MessageLayerRunner {
    inner: Box<dyn MessageLayer>,
    allowed: BTreeSet<ChannelId>,
}

impl MessageLayerRunner {
    fn new(inner: Box<dyn MessageLayer>, allowed: BTreeSet<ChannelId>) -> Self {
        Self { inner, allowed }
    }
}

impl Layer for MessageLayerRunner {
    fn identifier() -> LayerIdentifier
    where
        Self: Sized,
    {
        // static identifier isn't used for trait objects; unreachable in practice.
        "message_layer_runner".into()
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
pub struct LayerAssignment {
    pub channel_id: ChannelId,
    pub topic: String,
    pub encoding: String,
    pub schema_name: Option<String>,
    pub layer: LayerIdentifier,
}

/// A concrete execution plan for a given MCAP source.
pub struct ExecutionPlan {
    pub file_layers: Vec<Box<dyn Layer>>,
    pub runners: Vec<MessageLayerRunner>,
    pub assignments: Vec<LayerAssignment>,
    /// Optional lenses to apply as post-processing to chunks.
    pub lenses: Option<Lenses>,
}

impl ExecutionPlan {
    pub fn run(
        mut self,
        mcap_bytes: &[u8],
        summary: &mcap::Summary,
        emit: &mut dyn FnMut(Chunk),
    ) -> anyhow::Result<()> {
        // Create an emit wrapper that applies lenses if configured
        let mut emit_with_lenses = |chunk: Chunk| {
            if let Some(ref lenses) = self.lenses {
                for result in lenses.apply(&chunk) {
                    match result {
                        Ok(transformed_chunk) => emit(transformed_chunk),
                        Err(partial_chunk) => {
                            for error in partial_chunk.errors() {
                                re_log::error_once!("Lens error: {error}");
                            }
                            if let Some(chunk) = partial_chunk.take() {
                                emit(chunk);
                            }
                        }
                    }
                }
            } else {
                emit(chunk);
            }
        };

        for mut layer in self.file_layers {
            layer.process(mcap_bytes, summary, &mut emit_with_lenses)?;
        }

        for runner in &mut self.runners {
            runner.process(mcap_bytes, summary, &mut emit_with_lenses)?;
        }
        Ok(())
    }
}

/// Holds a set of all known layers, split into file-scoped and message-scoped.
pub struct LayerRegistry {
    file_factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn Layer>>,
    msg_factories: BTreeMap<LayerIdentifier, fn() -> Box<dyn MessageLayer>>,
    msg_order: Vec<LayerIdentifier>,
    fallback: Fallback,
    /// Factory for creating lenses to apply as post-processing to chunks.
    /// Using a factory because Lenses contains closures that can't be cloned.
    lenses_factory: Option<fn() -> Lenses>,
}

impl LayerRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self {
            file_factories: Default::default(),
            msg_factories: Default::default(),
            msg_order: Vec::new(),
            fallback: Fallback::None,
            lenses_factory: None,
        }
    }

    /// Configures a factory for creating lenses to apply as post-processing to chunks.
    ///
    /// Lenses transform raw message data (e.g., from protobuf) into semantic Rerun components.
    /// A factory function is used because lenses contain closures that cannot be cloned.
    pub fn with_lenses_factory(mut self, factory: fn() -> Lenses) -> Self {
        self.lenses_factory = Some(factory);
        self
    }

    /// Creates a registry with all builtin layers and raw fallback enabled.
    pub fn all_with_raw_fallback() -> Self {
        Self::all_builtin(true)
    }

    /// Creates a registry with all builtin layers and raw fallback disabled.
    pub fn all_without_raw_fallback() -> Self {
        Self::all_builtin(false)
    }

    /// Creates a registry with all builtin layers with configurable raw fallback.
    pub fn all_builtin(raw_fallback_enabled: bool) -> Self {
        let mut registry = Self::empty()
            // file layers:
            .register_file_layer::<McapRecordingInfoLayer>()
            .register_file_layer::<McapSchemaLayer>()
            .register_file_layer::<McapStatisticLayer>()
            // message layers (priority order):
            .register_message_layer::<McapRos2Layer>()
            .register_message_layer::<McapRos2ReflectionLayer>()
            .register_message_layer::<McapProtobufLayer>()
            // lenses for semantic transformations (e.g., Foxglove -> Rerun):
            .with_lenses_factory(foxglove_lenses);

        if raw_fallback_enabled {
            registry = registry
                .register_message_layer::<McapRawLayer>()
                .with_global_fallback::<McapRawLayer>();
        } else {
            // still register raw so users can explicitly select it, just no fallback
            registry = registry.register_message_layer::<McapRawLayer>();
        }

        registry
    }

    /// Register a file-scoped layer (runs once over the file/summary).
    pub fn register_file_layer<L: Layer + Default + 'static>(mut self) -> Self {
        let id = L::identifier();
        if self
            .file_factories
            .insert(id.clone(), || Box::new(L::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted file layer {} twice.", id);
        }
        self
    }

    /// Register a message-scoped layer (eligible to handle channels).
    pub fn register_message_layer<M: MessageLayer + Default + 'static>(mut self) -> Self {
        let id = <M as MessageLayer>::identifier();
        if self
            .msg_factories
            .insert(id.clone(), || Box::new(M::default()))
            .is_some()
        {
            re_log::warn_once!("Inserted message layer {} twice.", id);
        }
        self.msg_order.push(id);
        self
    }

    /// Configure a global fallback message layer (e.g. `raw`).
    pub fn with_global_fallback<M: MessageLayer + 'static>(mut self) -> Self {
        self.fallback = Fallback::Global(<M as MessageLayer>::identifier());
        self
    }

    /// Produce a filtered registry that only contains `selected` layers.
    pub fn select(&self, selected: &SelectedLayers) -> Self {
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
            // Preserve lenses factory when selecting layers
            lenses_factory: self.lenses_factory,
        }
    }

    fn select_fallback(&self, selected: &SelectedLayers) -> Fallback {
        match &self.fallback {
            Fallback::Global(id) if selected.contains(id) => Fallback::Global(id.clone()),
            Fallback::Global(_) | Fallback::None => Fallback::None,
        }
    }

    /// Build a concrete execution plan for a given file.
    pub fn plan(&self, summary: &mcap::Summary) -> anyhow::Result<ExecutionPlan> {
        let file_layers = self
            .file_factories
            .values()
            .map(|f| f())
            .collect::<Vec<_>>();

        // instantiate message layers and init them (supports_channel may depend on init)
        let mut msg_layers: Vec<(LayerIdentifier, Box<dyn MessageLayer>)> = self
            .msg_order
            .iter()
            .filter_map(|id| self.msg_factories.get(id).map(|f| (id.clone(), f())))
            .collect();

        for (_, l) in &mut msg_layers {
            l.init(summary)?;
        }

        let mut by_layer: BTreeMap<LayerIdentifier, BTreeSet<ChannelId>> = BTreeMap::new();
        let mut assignments: Vec<LayerAssignment> = Vec::new();

        for channel_id in summary.channels.values() {
            // explicit priority order
            let mut chosen: Option<LayerIdentifier> = None;
            for (id, layer) in &msg_layers {
                if layer.supports_channel(channel_id.as_ref()) {
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
                by_layer
                    .entry(id.clone())
                    .or_default()
                    .insert(ChannelId::from(channel_id.id));

                assignments.push(LayerAssignment {
                    channel_id: ChannelId::from(channel_id.id),
                    topic: channel_id.topic.clone(),
                    encoding: schema_encoding.to_owned(),
                    schema_name: channel_id.schema.as_ref().map(|s| s.name.clone()),
                    layer: id,
                });
            } else {
                re_log::debug!(
                    "No message layer selected for topic '{}' (encoding='{}', schema='{:?}')",
                    channel_id.topic,
                    schema_encoding,
                    schema_name,
                );
            }
        }

        let mut runners = Vec::new();
        for (layer_id, allowed) in by_layer {
            if let Some(factory) = self.msg_factories.get(&layer_id) {
                let inner = factory();
                runners.push(MessageLayerRunner::new(inner, allowed));
            }
        }

        Ok(ExecutionPlan {
            file_layers,
            runners,
            assignments,
            lenses: self.lenses_factory.map(|f| f()),
        })
    }
}
