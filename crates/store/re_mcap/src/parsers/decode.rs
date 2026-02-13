//! Utilities for decoding MCAP messages into Rerun chunks.

use re_chunk::external::nohash_hasher::{IntMap, IsEnabled};
use re_chunk::{
    Chunk, EntityPath, TimeColumn, TimeColumnBuilder, TimePoint, Timeline, TimelineName,
};
use re_log_types::TimeCell;

use crate::util::{TimestampCell, log_and_publish_timepoint_from_msg};

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
pub trait MessageParser {
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

    /// Returns the `TimePoint`s containing the log and publish times derived from the message.
    ///
    /// In most cases, this is a single time point and there is no need to override the default implementation.
    ///
    /// Exceptions are e.g. cases where the number of output rows differs from the number of input messages.
    /// For example, aggregate messages like `tf2_msgs/TFMessage` that can contain multiple transforms.
    fn get_log_and_publish_timepoints(
        &self,
        msg: &mcap::Message<'_>,
    ) -> anyhow::Result<Vec<re_chunk::TimePoint>> {
        Ok(vec![log_and_publish_timepoint_from_msg(msg)])
    }

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

    /// Add a timestamp to the timeline using the provided timestamp cell.
    ///
    /// The timeline name and [`TimeCell`] are automatically determined from the timestamp cell.
    /// For Unix epochs, creates a timestamp cell. For custom epochs, creates a duration cell.
    pub fn add_timestamp_cell(&mut self, timestamp_cell: TimestampCell) -> &mut Self {
        let timeline_name = TimelineName::from(timestamp_cell.timeline_name());
        let cell = timestamp_cell.into_time_cell();

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
