//! Utilities for decoding MCAP messages into Rerun chunks.

use anyhow::Context as _;

use re_chunk::{
    Chunk, EntityPath, TimeColumn, TimeColumnBuilder, TimePoint, Timeline, TimelineName,
    external::nohash_hasher::{IntMap, IsEnabled},
};
use re_log_types::TimeCell;
use thiserror::Error;

pub type SchemaName = String;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Channel {0} does not define a schema")]
    NoSchema(String),

    #[error("Invalid schema {schema}: {source}")]
    InvalidSchema {
        schema: String,
        source: anyhow::Error,
    },

    #[error("No schema loader support for schema: {0}")]
    UnsupportedSchema(SchemaName),

    #[error(transparent)]
    Mcap(#[from] ::mcap::McapError),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChannelId(pub u16);

impl From<u16> for ChannelId {
    fn from(id: u16) -> Self {
        Self(id)
    }
}

impl IsEnabled for ChannelId {}

pub(crate) type Parser = (ParserContext, Box<dyn McapMessageParser>);

/// Decodes batches of messages from an MCAP into Rerun chunks using previously registered parsers.
pub struct McapChunkDecoder {
    parsers: IntMap<ChannelId, Parser>,
}

impl McapChunkDecoder {
    pub fn new(parsers: IntMap<ChannelId, Parser>) -> Self {
        Self { parsers }
    }

    /// Decode the next message in the chunk
    pub fn decode_next(&mut self, msg: &::mcap::Message<'_>) -> Result<(), PluginError> {
        re_tracing::profile_function!();

        let channel = msg.channel.as_ref();
        let channel_id = ChannelId(channel.id);
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

        if let Some((ctx, parser)) = self.parsers.get_mut(&channel_id) {
            ctx.add_timepoint(timepoint.clone());

            parser
                .append(ctx, msg)
                .with_context(|| {
                    format!(
                        "Failed to append message for topic: {} of type: {}",
                        channel.topic, schema.name
                    )
                })
                .map_err(PluginError::Other)?;
        } else {
            // TODO(#10867): If we encounter a message that we can't parse at all we should emit a warning.
            // Note that this quite easy to achieve when using layers and only selecting a subset.
            // However, to not overwhelm the user this should be reported in a _single_ static chunk,
            // so this is not the right place for this. Maybe we need to introduce something like a "report".
        }
        Ok(())
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
