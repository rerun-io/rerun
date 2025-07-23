//! Utilities for decoding MCAP messages into Rerun chunks.

use std::{collections::HashMap, sync::Arc};

use anyhow::Context as _;
use mcap::Message;

use re_chunk::{
    Chunk, EntityPath, TimeColumn, TimeColumnBuilder, TimePoint, Timeline, TimelineName,
    external::nohash_hasher::{IntMap, IsEnabled},
};
use re_log_types::TimeCell;
use re_sorbet::SorbetSchema;
use thiserror::Error;

use super::schema::UnsupportedSchemaMessageParser;

pub type SchemaName = String;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Channel {0} does not define a schema")]
    NoSchema(String),

    #[error("No schema loader support for schema: {0}")]
    UnsupportedSchema(SchemaName),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Trait for plugins that can parse MCAP schemas and create message parsers.
///
/// A [`SchemaPlugin`] is responsible for handling a specific type of schema found in MCAP files.
/// Each plugin knows how to parse the schema definition and create appropriate message parsers
/// that can decode messages conforming to that schema into Rerun chunks.
pub trait SchemaPlugin {
    /// Returns the name of the schema this plugin handles.
    ///
    /// This name is used as the key in the [`MessageDecoderRegistry`] to match
    /// incoming MCAP messages with their appropriate parser plugin.
    ///
    /// Example schema names:
    /// - `"sensor_msgs/msg/Image"`
    /// - `"sensor_msgs/msg/CompressedImage"`
    /// - `"geometry_msgs/msg/PoseStamped"`
    /// - `"nav_msgs/msg/OccupancyGrid"`
    ///
    /// The name should exactly match the schema name found in the MCAP file.
    fn name(&self) -> SchemaName;

    /// Parses the schema definition from an MCAP channel, and returns a [`SorbetSchema`].
    fn parse_schema(&self, channel: &mcap::Channel<'_>) -> Result<SorbetSchema, PluginError>;

    /// Creates a new [`McapMessageParser`] instance for processing messages from this channel.
    ///
    /// This method is called once per channel/entity path combination when the first
    /// message for that channel is encountered. The returned parser will handle all
    /// subsequent messages for that specific channel.
    ///
    /// ### Performance Considerations
    ///
    /// The `num_rows` argument allows parsers to pre-allocate storage with the
    /// correct capacity, avoiding reallocations during message processing:
    ///
    /// ```rust
    /// # use anyhow::Error;
    /// # use mcap::Channel;
    /// # use re_chunk::Chunk;
    /// # use re_sorbet::SorbetSchema;
    /// # use re_data_loader::mcap::decode::{
    /// #     McapMessageParser, ParserContext, PluginError, SchemaName, SchemaPlugin,
    /// # };
    /// # struct MyParser {
    /// #     data: Vec<u8>,
    /// #     timestamps: Vec<u64>,
    /// # }
    /// # impl McapMessageParser for MyParser {
    /// #     fn append(
    /// #         &mut self,
    /// #         _ctx: &mut ParserContext,
    /// #         _message: &mcap::Message<'_>,
    /// #     ) -> Result<(), Error> {
    /// #         Ok(())
    /// #     }
    /// #     fn finalize(self: Box<Self>, _ctx: ParserContext) -> Result<Vec<Chunk>, Error> {
    /// #         Ok(vec![])
    /// #     }
    /// # }
    /// # struct MyPlugin;
    /// # impl SchemaPlugin for MyPlugin {
    /// #     fn name(&self) -> SchemaName {
    /// #         "my_schema".into()
    /// #     }
    /// #     fn parse_schema(&self, _channel: &Channel<'_>) -> Result<SorbetSchema, PluginError> {
    /// #         unreachable!()
    /// #     }
    /// fn create_message_parser(
    ///     &self,
    ///     _channel: &Channel<'_>,
    ///     num_rows: usize,
    /// ) -> Box<dyn McapMessageParser> {
    ///     Box::new(MyParser {
    ///         data: Vec::with_capacity(num_rows),
    ///         timestamps: Vec::with_capacity(num_rows),
    ///         // … other fields
    ///     })
    /// }
    /// # }
    /// ```
    fn create_message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Box<dyn McapMessageParser>;
}

/// Trait for parsing MCAP messages of a specific schema into Rerun chunks.
///
/// This trait defines the interface for converting MCAP messages into Rerun's internal
/// chunk format. Implementations handle the incremental processing of messages and
/// eventual conversion to structured data that can be visualized in Rerun.
///
/// ### Message Parsing
///
/// The parsing process follows a two-phase approach:
///
/// 1. Process messages incrementally via [`append()`](`Self::append`),
///    where parsers can extract and accumulate data from each message.
/// 2. All accumulated data is converted into [`Chunk`]s via
///    [`finalize()`](`Self::finalize`), which consumes the parser and returns the final Rerun chunks.
pub trait McapMessageParser {
    /// Process a single MCAP message and accumulate its data.
    ///
    /// This method is called for each message in the MCAP file that belongs to the
    /// associated channel/schema.
    ///
    /// Implementations should:
    ///
    /// 1. Decode the message data according to the schema
    /// 2. Extract any _additional_ timestamp information and add it to the [`ParserContext`].
    ///    Note: `log_time` and `publish_time` are added automatically.
    /// 3. Accumulate the decoded data for later conversion to Rerun [`Chunk`]s in [`finalize()`](`Self::finalize`).
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()>;

    /// Consume the parser and convert all accumulated data into Rerun chunks.
    ///
    /// This method is called after all messages have been processed via [`append()`](`Self::append`).
    /// It should convert the accumulated data into one or more [`Chunk`]s.
    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>>;
}

/// Registry of message decoders for different schemas.
#[derive(Default)]
pub struct MessageDecoderRegistry(HashMap<SchemaName, Arc<dyn SchemaPlugin>>);

impl MessageDecoderRegistry {
    /// Create a new empty registry.
    ///
    /// The registry starts without any plugins registered. You'll need to add plugins
    /// using [`register`](Self::register) or [`register_default`](Self::register_default)
    /// before it can handle any message types.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Register a new schema plugin.
    pub fn register<T: SchemaPlugin + 'static>(&mut self, plugin: T) -> &mut Self {
        self.0.insert(plugin.name(), Arc::new(plugin));
        self
    }

    /// Registers a new schema plugin using its [`Default`] implementation.
    pub fn register_default<T: SchemaPlugin + Default + 'static>(&mut self) -> &mut Self {
        self.register(T::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(pub u16);

impl From<u16> for ChannelId {
    fn from(id: u16) -> Self {
        Self(id)
    }
}

impl From<ChannelId> for u16 {
    fn from(id: ChannelId) -> Self {
        id.0
    }
}

impl IsEnabled for ChannelId {}

/// Decodes batches of messages from an MCAP into Rerun chunks using previously registered parsers.
pub struct McapChunkDecoder<'a> {
    registry: &'a MessageDecoderRegistry,
    channel_counts: IntMap<ChannelId, usize>,
    parsers: IntMap<EntityPath, (ParserContext, Box<dyn McapMessageParser>)>,
}

impl<'a> McapChunkDecoder<'a> {
    pub fn new(
        registry: &'a MessageDecoderRegistry,
        channel_counts: IntMap<ChannelId, usize>,
    ) -> Self {
        Self {
            registry,
            channel_counts,
            parsers: IntMap::default(),
        }
    }

    /// Decode the next message in the chunk
    pub fn decode_next(&mut self, msg: &Message<'_>) -> Result<(), PluginError> {
        let channel = msg.channel.as_ref();
        let entity_path = EntityPath::from(channel.topic.as_str());
        let timepoint = TimePoint::from([
            (
                "log_time",
                TimeCell::from_timestamp_nanos_since_epoch(msg.log_time as i64),
            ),
            (
                "publish_time",
                TimeCell::from_timestamp_nanos_since_epoch(msg.publish_time as i64),
            ),
        ]);

        let schema = channel.schema.as_ref();

        let Some(schema) = schema else {
            // TODO(harold): in the future, we might want to try sensible decodings with heuristics like the name.
            // Schemaless messages are not supported yet.
            re_log::warn_once!("Found schemaless message for {entity_path}");
            return Err(PluginError::NoSchema(channel.topic.clone()));
        };

        let num_rows = *self
            .channel_counts
            .get(&ChannelId(channel.id))
            .unwrap_or_else(|| {
                re_log::warn_once!(
                    "No message count for topic {}. This is unexpected.",
                    channel.topic
                );

                &0
            });

        let Some(plugin) = self.registry.0.get(&schema.name) else {
            let mcap::Schema {
                id: _,
                name,
                encoding: _,
                data: _,
            } = schema.as_ref();

            re_log::warn_once!("No loader for schema {name:?}");

            let (ctx, parser) = self.parsers.entry(entity_path.clone()).or_insert_with(|| {
                (
                    ParserContext::new(entity_path.clone()),
                    Box::new(UnsupportedSchemaMessageParser::new(num_rows)),
                )
            });

            ctx.add_timepoint(timepoint);

            return parser
                .append(ctx, msg)
                .with_context(|| "Failed to append unsupported schema message")
                .map_err(PluginError::Other);
        };

        // TODO(#10724): Add support for logging warnings directly to Rerun
        let (ctx, parser) = self.parsers.entry(entity_path.clone()).or_insert_with(|| {
            (
                ParserContext::new(entity_path.clone()),
                plugin.create_message_parser(channel, num_rows),
            )
        });

        ctx.add_timepoint(timepoint);
        parser
            .append(ctx, msg)
            .with_context(|| {
                format!(
                    "Failed to append message for topic: {} of type: {}",
                    channel.topic, schema.name
                )
            })
            .map_err(PluginError::Other)
    }

    /// Finish the decoding process and return the chunks.
    pub fn finish(self) -> impl Iterator<Item = Result<Chunk, PluginError>> {
        self.parsers
            .into_values()
            .flat_map(|(ctx, parser)| match parser.finalize(ctx) {
                Ok(chunks) => chunks.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(err) => vec![Err(PluginError::Other(err))],
            })
    }
}

/// Common context used by parsers to build timelines and store entity paths.
pub struct ParserContext {
    entity_path: EntityPath,
    pub timelines: IntMap<TimelineName, TimeColumnBuilder>,
}

impl ParserContext {
    /// Construct a new parser context with the given [`EntityPath`].
    pub fn new(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            timelines: IntMap::default(),
        }
    }

    /// Add an additional [`TimePoint`] to the timelines in this context.
    ///
    /// # Note
    ///
    /// The `log_time` and `publish_time` are added to the timelines automatically,
    /// this function allows you to add additional timepoints such as sensor timestamps.
    pub fn add_timepoint(&mut self, timepoint: TimePoint) -> &mut Self {
        for (timeline, cell) in timepoint {
            self.timelines
                .entry(timeline)
                .or_insert_with(|| TimeColumn::builder(Timeline::new(timeline, cell.typ)))
                .with_row(cell.value);
        }

        self
    }

    /// Add a single [`TimeCell`] to the [`Timeline`] with the given name.
    ///
    /// # Note
    ///
    /// The `log_time` and `publish_time` are added to the timelines automatically,
    /// this function allows you to add additional timepoints such as sensor timestamps.
    pub fn add_time_cell(
        &mut self,
        timeline_name: impl Into<TimelineName>,
        cell: TimeCell,
    ) -> &mut Self {
        let timeline_name = timeline_name.into();
        self.timelines
            .entry(timeline_name)
            .or_insert_with(|| TimeColumn::builder(Timeline::new(timeline_name, cell.typ)))
            .with_row(cell.value);

        self
    }

    /// Consume this context and build all timelines into [`TimeColumn`]s.
    pub fn build_timelines(self) -> IntMap<TimelineName, TimeColumn> {
        self.timelines
            .into_iter()
            .map(|(name, builder)| (name, builder.build()))
            .collect()
    }

    /// Get the entity path associated with this context.
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }
}
