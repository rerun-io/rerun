use arrow::array::{ListBuilder, UInt8Builder};
use re_chunk::{ChunkId, external::arrow::array::FixedSizeListBuilder};
use re_types::{Component as _, ComponentDescriptor, components};

use crate::mcap::decode::{McapMessageParser, ParserContext, PluginError};

use super::blob_list_builder;

pub struct RawMcapMessageParser {
    data: FixedSizeListBuilder<ListBuilder<UInt8Builder>>,
}

impl RawMcapMessageParser {
    pub const ARCHETYPE_NAME: &str = "rerun.mcap.Message";

    pub fn new(num_rows: usize) -> Self {
        Self {
            data: blob_list_builder(num_rows),
        }
    }
}

impl McapMessageParser for RawMcapMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        self.data.values().values().append_slice(&msg.data);
        self.data.values().append(true);
        self.data.append(true);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self { mut data } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let chunk = re_chunk::Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            std::iter::once((
                ComponentDescriptor {
                    archetype: Some(Self::ARCHETYPE_NAME.into()),
                    component: "data".into(),
                    component_type: Some(components::Blob::name()),
                },
                data.finish().into(),
            ))
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        Ok(vec![chunk])
    }
}
