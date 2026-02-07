use arrow::array::{ListBuilder, UInt8Builder};
use re_chunk::ChunkId;
use re_chunk::external::arrow::array::FixedSizeListBuilder;
use re_sdk_types::archetypes::McapMessage;

use crate::parsers::util::blob_list_builder;
use crate::parsers::{MessageParser, ParserContext};
use crate::{Error, LayerIdentifier, MessageLayer};

struct RawMcapMessageParser {
    data: FixedSizeListBuilder<ListBuilder<UInt8Builder>>,
}

impl RawMcapMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            data: blob_list_builder(num_rows),
        }
    }
}

impl MessageParser for RawMcapMessageParser {
    fn append(
        &mut self,
        _ctx: &mut ParserContext,
        msg: &::mcap::Message<'_>,
    ) -> anyhow::Result<()> {
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
            std::iter::once((McapMessage::descriptor_data(), data.finish().into())).collect(),
        )
        .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        Ok(vec![chunk])
    }
}

/// Logs the raw, encoded bytes of arbitrary MCAP messages as Rerun blobs.
///
/// The result will be verbatim copies of the original messages without decoding
/// or imposing any semantic meaning on the data.
#[derive(Default, Debug)]
pub struct McapRawLayer;

impl MessageLayer for McapRawLayer {
    fn identifier() -> LayerIdentifier {
        "raw".into()
    }

    fn init(&mut self, summary: &::mcap::Summary) -> Result<(), Error> {
        let _ = summary; // nothing to do
        Ok(())
    }

    fn supports_channel(&self, _channel: &mcap::Channel<'_>) -> bool {
        // Raw can capture any channel
        true
    }

    fn message_parser(
        &self,
        _channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        Some(Box::new(RawMcapMessageParser::new(num_rows)))
    }
}
