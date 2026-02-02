use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::TextDocument;

use super::super::definitions::std_msgs;
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::{MessageParser, ParserContext, cdr};

/// Plugin that parses `std_msgs/msg/String` messages.
pub struct StringMessageParser {
    /// The text content from String messages.
    texts: Vec<String>,
}

impl Ros2MessageParser for StringMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            texts: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for StringMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let std_msgs::StringMessage { data } =
            cdr::try_decode_message::<std_msgs::StringMessage>(&msg.data)?;
        self.texts.push(data);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let Self { texts } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let text_documents = TextDocument::update_fields()
            .with_many_text(texts)
            .columns_of_unit_batches()?
            .collect();

        let chunk =
            Chunk::from_auto_row_ids(ChunkId::new(), entity_path, timelines, text_documents)?;

        Ok(vec![chunk])
    }
}
