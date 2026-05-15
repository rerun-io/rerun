use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::TextDocument;

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::std_msgs;
use crate::parsers::ros1msg::wire::Ros1Reader;

pub struct StringMessageParser {
    texts: Vec<String>,
}

impl Ros1MessageParser for StringMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            texts: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for StringMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let mut reader = Ros1Reader::new(&msg.data);
        let message = std_msgs::StringMessage::read(&mut reader)?;
        reader.finish()?;
        self.texts.push(message.data);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            ctx.entity_path().clone(),
            ctx.build_timelines(),
            TextDocument::update_fields()
                .with_many_text(self.texts)
                .columns_of_unit_batches()?
                .collect(),
        )?;

        Ok(vec![chunk])
    }
}
