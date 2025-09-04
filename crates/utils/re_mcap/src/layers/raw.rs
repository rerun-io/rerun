use arrow::array::{ListBuilder, UInt8Builder};
use re_chunk::{ChunkId, external::arrow::array::FixedSizeListBuilder};
use re_types::archetypes::McapMessage;

use crate::{
    Error, LayerIdentifier, MessageLayer,
    layers::{McapProtobufLayer, McapRos2Layer},
    parsers::{MessageParser, ParserContext, util::blob_list_builder},
};

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
///
/// When used as a fallback layer, it only processes channels that cannot be handled
/// by other semantic layers (protobuf and ROS2).
#[derive(Debug)]
pub struct McapRawLayer {
    /// Whether to act as a fallback layer for channels not handled by semantic layers.
    fallback_enabled: bool,

    /// Protobuf layer used to check channel support when in fallback mode.
    protobuf_layer: McapProtobufLayer,

    /// ROS2 layer used to check channel support when in fallback mode.
    ros2_layer: McapRos2Layer,
}

impl Default for McapRawLayer {
    fn default() -> Self {
        Self {
            fallback_enabled: true,
            protobuf_layer: McapProtobufLayer::default(),
            ros2_layer: McapRos2Layer::default(),
        }
    }
}

impl McapRawLayer {
    /// Enables or disables fallback mode for the raw layer.
    pub fn with_fallback_enabled(mut self, enabled: bool) -> Self {
        self.fallback_enabled = enabled;
        self
    }
}

impl MessageLayer for McapRawLayer {
    fn identifier() -> LayerIdentifier {
        "raw".into()
    }

    fn init(&mut self, summary: &::mcap::Summary) -> Result<(), Error> {
        if self.fallback_enabled {
            self.protobuf_layer.init(summary)?;
            self.ros2_layer.init(summary)?;
        }
        Ok(())
    }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        if !self.fallback_enabled {
            return true;
        }

        // In fallback mode, only handle channels that semantic layers cannot handle
        !self.protobuf_layer.supports_channel(channel) && !self.ros2_layer.supports_channel(channel)
    }

    fn message_parser(
        &self,
        _channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        if !self.supports_channel(_channel) {
            return None;
        }

        Some(Box::new(RawMcapMessageParser::new(num_rows)))
    }
}
