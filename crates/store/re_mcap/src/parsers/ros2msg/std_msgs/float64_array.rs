use re_chunk::external::arrow::array::{Float64Builder, ListBuilder};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::Scalars;

use super::super::definitions::std_msgs;
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::{MessageParser, ParserContext, cdr};

/// Plugin that parses `std_msgs/msg/Float64Array` messages.
pub struct Float64ArrayMessageParser {
    /// The array data from `Float64Array` messages.
    arrays: ListBuilder<Float64Builder>,
}

impl Ros2MessageParser for Float64ArrayMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            arrays: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
        }
    }
}

impl MessageParser for Float64ArrayMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let std_msgs::Float64ArrayMessage { data } =
            cdr::try_decode_message::<std_msgs::Float64ArrayMessage>(&msg.data)?;
        self.arrays.values().append_slice(data.as_slice());
        self.arrays.append(true);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let Self { mut arrays } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((Scalars::descriptor_scalars(), arrays.finish())).collect(),
        )?;

        Ok(vec![chunk])
    }
}
