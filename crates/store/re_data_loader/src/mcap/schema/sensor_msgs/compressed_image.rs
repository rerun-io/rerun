use re_chunk::{
    Chunk, ChunkId, RowId, TimePoint,
    external::arrow::array::{FixedSizeListBuilder, StringBuilder},
};
use re_log_types::TimeCell;
use re_mcap_ros2::sensor_msgs;
use re_sorbet::SorbetSchema;
use re_types::{
    ComponentDescriptor,
    archetypes::{EncodedImage, VideoStream},
    components::VideoCodec,
};

use crate::mcap::{
    cdr,
    decode::{McapMessageParser, ParserContext, PluginError, SchemaName, SchemaPlugin},
};

/// Plugin that parses `sensor_msgs/msg/CompressedImage` messages.
#[derive(Default)]
pub struct CompressedImageSchemaPlugin;

impl SchemaPlugin for CompressedImageSchemaPlugin {
    fn name(&self) -> SchemaName {
        "sensor_msgs/msg/CompressedImage".into()
    }

    fn parse_schema(&self, _channel: &mcap::Channel<'_>) -> Result<SorbetSchema, PluginError> {
        Err(PluginError::Other(anyhow::anyhow!("todo")))
    }

    fn create_message_parser(
        &self,
        _channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Box<dyn McapMessageParser> {
        Box::new(CompressedImageMessageParser::new(num_rows)) as Box<dyn McapMessageParser>
    }
}

pub struct CompressedImageMessageParser {
    /// The raw image data blobs.
    ///
    /// Note: These blobs are directly moved into a `Blob`, without copying.
    blobs: Vec<Vec<u8>>,
    formats: FixedSizeListBuilder<StringBuilder>,
    is_h264: bool,
}

impl CompressedImageMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.CompressedImage";

    pub fn new(num_rows: usize) -> Self {
        Self {
            blobs: Vec::with_capacity(num_rows),
            formats: FixedSizeListBuilder::with_capacity(StringBuilder::new(), 1, num_rows),
            is_h264: false,
        }
    }
}

impl McapMessageParser for CompressedImageMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::CompressedImage {
            header,
            data,
            format,
        } = cdr::try_decode_message::<sensor_msgs::CompressedImage<'_>>(&msg.data)?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(header.stamp.as_nanos()),
        );

        self.blobs.push(data.into_owned());

        if format.eq_ignore_ascii_case("h264") {
            // If the format for this topic is h264 once, we assume it is h264 for all messages.
            self.is_h264 = true;
        }

        self.formats.values().append_value(format.as_str());
        self.formats.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        let Self {
            blobs,
            mut formats,
            is_h264,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let components = if is_h264 {
            VideoStream::update_fields()
                .with_many_sample(blobs)
                .columns_of_unit_batches()
                .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?
                .collect()
        } else {
            EncodedImage::update_fields()
                .with_many_blob(blobs)
                .columns_of_unit_batches()
                .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?
                .collect()
        };

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            components,
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        let meta_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            std::iter::once((
                ComponentDescriptor::partial("format").with_archetype(Self::ARCHETYPE_NAME.into()),
                formats.finish().into(),
            ))
            .collect(),
        )
        .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;

        if is_h264 {
            // codec should be logged once per entity, as static data.
            let codec_chunk = Chunk::builder(entity_path.clone())
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &VideoStream::update_fields().with_codec(VideoCodec::H264),
                )
                .build()
                .map_err(|err| PluginError::Other(anyhow::anyhow!(err)))?;
            Ok(vec![chunk, meta_chunk, codec_chunk])
        } else {
            Ok(vec![chunk, meta_chunk])
        }
    }
}
