use anyhow::Context as _;
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, DepthImage, Image};

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::sensor_msgs;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::parsers::ros2msg::sensor_msgs::decode_image_encoding;
use crate::parsers::ros2msg::util::suffix_image_plane_frame_ids;

pub struct ImageMessageParser {
    blobs: Vec<Vec<u8>>,
    formats: Vec<re_sdk_types::datatypes::ImageFormat>,
    is_depth_image: bool,
    frame_ids: Vec<String>,
}

impl Ros1MessageParser for ImageMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            blobs: Vec::with_capacity(num_rows),
            formats: Vec::with_capacity(num_rows),
            is_depth_image: false,
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for ImageMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let mut reader = Ros1Reader::new(&msg.data);
        let image = sensor_msgs::Image::read(&mut reader)
            .context("Failed to decode sensor_msgs/Image message from ROS1 data")?;
        reader.finish()?;

        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros1(
            image.header.stamp.as_nanos(),
            ctx.time_type(),
        ));

        let dimensions = [image.width, image.height];
        let encoding = decode_image_encoding(&image.encoding).with_context(|| {
            format!(
                "Failed to decode image format for encoding '{}' with dimensions {}x{}",
                image.encoding, image.width, image.height
            )
        })?;

        self.is_depth_image = encoding.is_single_channel();
        self.frame_ids.push(image.header.frame_id);
        self.blobs.push(image.data);
        self.formats.push(encoding.to_image_format(dimensions));
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            blobs,
            formats,
            is_depth_image,
            frame_ids,
        } = *self;

        let mut components: Vec<_> = if is_depth_image {
            DepthImage::update_fields()
                .with_many_buffer(blobs)
                .with_many_format(formats)
                .columns_of_unit_batches()?
                .collect()
        } else {
            Image::update_fields()
                .with_many_buffer(blobs)
                .with_many_format(formats)
                .columns_of_unit_batches()?
                .collect()
        };

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(suffix_image_plane_frame_ids(frame_ids))
                .columns_of_unit_batches()?,
        );

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            ctx.entity_path().clone(),
            ctx.build_timelines(),
            components.into_iter().collect(),
        )?])
    }
}
