use anyhow::Context as _;
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, DepthImage, Image};
use re_sdk_types::datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};

use super::super::Ros2MessageParser;
use super::super::util::suffix_image_plane_frame_ids;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros2msg::definitions::sensor_msgs;

pub struct ImageMessageParser {
    /// The raw image data blobs.
    ///
    /// Note: These blobs are directly moved into a `Blob`, without copying.
    blobs: Vec<Vec<u8>>,
    image_formats: Vec<ImageFormat>,
    is_depth_image: bool,
    frame_ids: Vec<String>,
}

impl Ros2MessageParser for ImageMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            blobs: Vec::with_capacity(num_rows),
            image_formats: Vec::with_capacity(num_rows),
            is_depth_image: false,
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for ImageMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let sensor_msgs::Image {
            header,
            data,
            height,
            width,
            encoding,
            ..
        } = cdr::try_decode_message::<sensor_msgs::Image<'_>>(&msg.data)
            .context("Failed to decode sensor_msgs::Image message from CDR data")?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.push(header.frame_id);

        let dimensions = [width, height];
        let img_format = decode_image_format(&encoding, dimensions)
            .with_context(|| format!("Failed to decode image format for encoding '{encoding}' with dimensions {width}x{height}"))?;

        // TODO(#10726): big assumption here: image format can technically be different for each image on the topic.
        // `color_model` is `None` for formats created with `ImageFormat::depth`
        self.is_depth_image = img_format.color_model.is_none();

        self.blobs.push(data.into_owned());
        self.image_formats.push(img_format);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            blobs,
            image_formats,
            is_depth_image,
            frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let mut chunk_components: Vec<_> = if is_depth_image {
            DepthImage::update_fields()
                .with_many_buffer(blobs)
                .with_many_format(image_formats)
                .columns_of_unit_batches()?
                .collect()
        } else {
            Image::update_fields()
                .with_many_buffer(blobs)
                .with_many_format(image_formats)
                .columns_of_unit_batches()?
                .collect()
        };

        // We need a frame ID for the image plane. This doesn't exist in ROS,
        // so we use the camera frame ID with a suffix here (see also camera info parser).
        let image_plane_frame_ids = suffix_image_plane_frame_ids(frame_ids);
        chunk_components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(image_plane_frame_ids)
                .columns_of_unit_batches()?,
        );

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            chunk_components.into_iter().collect(),
        )?])
    }
}

fn decode_image_format(encoding: &str, dimensions: [u32; 2]) -> anyhow::Result<ImageFormat> {
    match encoding {
        "rgb8" => Ok(ImageFormat::rgb8(dimensions)),
        "rgba8" => Ok(ImageFormat::rgba8(dimensions)),
        "rgb16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::RGB,
            ChannelDatatype::U16,
        )),
        "rgba16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::RGBA,
            ChannelDatatype::U16,
        )),
        "bgr8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGR,
            ChannelDatatype::U8,
        )),
        "bgra8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGRA,
            ChannelDatatype::U8,
        )),
        "bgr16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGR,
            ChannelDatatype::U16,
        )),
        "bgra16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGRA,
            ChannelDatatype::U16,
        )),
        "mono8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::L,
            ChannelDatatype::U8,
        )),
        "mono16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::L,
            ChannelDatatype::U16,
        )),
        "yuyv" | "yuv422_yuy2" => Ok(ImageFormat::from_pixel_format(
            dimensions,
            PixelFormat::YUY2,
        )),
        "nv12" => Ok(ImageFormat::from_pixel_format(
            dimensions,
            PixelFormat::NV12,
        )),
        // Depth image formats
        "8UC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::U8)),
        "8SC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I8)),
        "16UC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::U16)),
        "16SC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I16)),
        "32SC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I32)),
        "32FC1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::F32)),
        // Other
        format => {
            anyhow::bail!(
                "Unsupported image encoding '{format}'. Supported encodings include: rgb8, rgba8, rgb16, rgba16, bgr8, bgra8, bgr16, bgra16, mono8, mono16, yuyv, yuv422_yuy2, nv12, 8UC1, 8SC1, 16UC1, 16SC1, 32SC1, 32FC1"
            )
        }
    }
}
