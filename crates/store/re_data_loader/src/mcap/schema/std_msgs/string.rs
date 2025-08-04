use re_chunk::{Chunk, ChunkId};
use re_mcap_ros2::std_msgs;
use re_types::archetypes::TextDocument;

use crate::mcap::{
    cdr,
    decode::{McapMessageParser, ParserContext, PluginError, SchemaName, SchemaPlugin},
};

/// Plugin that parses `std_msgs/msg/String` messages.
#[derive(Default)]
pub struct StringSchemaPlugin;

impl SchemaPlugin for StringSchemaPlugin {
    fn name(&self) -> SchemaName {
        "std_msgs/msg/String".into()
    }

    fn create_message_parser(
        &self,
        _channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Box<dyn McapMessageParser> {
        Box::new(StringMessageParser::new(num_rows)) as Box<dyn McapMessageParser>
    }
}

pub struct StringMessageParser {
    /// The text content from String messages.
    texts: Vec<String>,
}

impl StringMessageParser {
    pub fn new(num_rows: usize) -> Self {
        Self {
            texts: Vec::with_capacity(num_rows),
        }
    }
}

impl McapMessageParser for StringMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let string_msg = cdr::try_decode_message::<std_msgs::StringMessage>(&msg.data)?;
        self.texts.push(string_msg.data);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let Self { texts } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let text_documents = TextDocument::update_fields()
            .with_many_text(texts)
            .columns_of_unit_batches()
            .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?
            .collect();

        let chunk =
            Chunk::from_auto_row_ids(ChunkId::new(), entity_path, timelines, text_documents)
                .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![chunk])
    }
}
