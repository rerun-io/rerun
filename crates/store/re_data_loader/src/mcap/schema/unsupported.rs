use re_chunk::{
    ChunkId,
    external::arrow::array::{FixedSizeListBuilder, StringBuilder, UInt16Builder},
};
use re_types::{ComponentDescriptor, archetypes};

use crate::mcap::decode::{McapMessageParser, ParserContext, PluginError};

pub struct UnsupportedSchemaMessageParser {
    schema_ids: FixedSizeListBuilder<UInt16Builder>,
    schema_names: FixedSizeListBuilder<StringBuilder>,
    encodings: FixedSizeListBuilder<StringBuilder>,
    text_log_msg: FixedSizeListBuilder<StringBuilder>,
    text_log_level: FixedSizeListBuilder<StringBuilder>,
}

impl UnsupportedSchemaMessageParser {
    pub const ARCHETYPE_NAME: &str = "rerun_mcap.UnsupportedSchema";

    pub fn new(num_rows: usize) -> Self {
        Self {
            schema_ids: FixedSizeListBuilder::with_capacity(UInt16Builder::new(), 1, num_rows),
            schema_names: FixedSizeListBuilder::with_capacity(StringBuilder::new(), 1, num_rows),
            encodings: FixedSizeListBuilder::with_capacity(StringBuilder::new(), 1, num_rows),
            //
            text_log_msg: FixedSizeListBuilder::with_capacity(StringBuilder::new(), 1, num_rows),
            text_log_level: FixedSizeListBuilder::with_capacity(StringBuilder::new(), 1, num_rows),
        }
    }
}

impl McapMessageParser for UnsupportedSchemaMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        if let Some(schema) = msg.channel.schema.as_ref() {
            let schema_name = schema.name.as_str();
            self.schema_ids.values().append_value(schema.id);
            self.schema_names.values().append_value(schema_name);
            self.text_log_msg
                .values()
                .append_value(format!("Unsupported schema: {schema_name}"));
        } else {
            self.schema_ids.values().append_null();
            self.schema_names.values().append_null();
            self.text_log_msg
                .values()
                .append_value("Unsupported message");
        };

        self.encodings
            .values()
            .append_value(msg.channel.message_encoding.as_str());

        self.text_log_level.values().append_value("WARN");

        self.schema_ids.append(true);
        self.schema_names.append(true);
        self.encodings.append(true);

        self.text_log_msg.append(true);
        self.text_log_level.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let Self {
            mut schema_ids,
            mut schema_names,
            mut encodings,
            mut text_log_msg,
            mut text_log_level,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let log_chunk = re_chunk::Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            Default::default(),
            [
                (
                    archetypes::TextLog::descriptor_text(),
                    text_log_msg.finish().into(),
                ),
                (
                    archetypes::TextLog::descriptor_level(),
                    text_log_level.finish().into(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        let chunk = re_chunk::Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            [
                (
                    ComponentDescriptor::partial("schema_id")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    schema_ids.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("schema_name")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    schema_names.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("encoding")
                        .with_archetype(Self::ARCHETYPE_NAME.into()),
                    encodings.finish().into(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![log_chunk, chunk])
    }
}
