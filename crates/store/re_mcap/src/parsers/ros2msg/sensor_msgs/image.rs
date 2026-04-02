use anyhow::Context as _;
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, DepthImage, Image};
use re_sdk_types::datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};

use super::super::Ros2MessageParser;
use super::super::util::spatial_camera_frame_ids_or_log_missing;
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
        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros2(
            header.stamp.as_nanos() as u64,
            ctx.time_type(),
        ));

        self.frame_ids.push(header.frame_id);

        let dimensions = [width, height];
        let img_encoding = decode_image_encoding(&encoding)
            .with_context(|| format!("Failed to decode image format for encoding '{encoding}' with dimensions {width}x{height}"))?;

        // We assume that images with a single channel encoding (e.g. `16UC1`) are depth images, and all others are regular color images.
        self.is_depth_image = img_encoding.is_single_channel();

        self.blobs.push(data.into_owned());
        self.image_formats
            .push(img_encoding.to_image_format(dimensions));

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
        let spatial_frame_ids = spatial_camera_frame_ids_or_log_missing(
            ctx.channel_topic(),
            &entity_path,
            "sensor_msgs/msg/Image",
            "Importing the topic as plain 2D image data.",
            frame_ids,
        );
        let timelines = ctx.build_timelines();

        // TODO(#10726): big assumption here: image format can technically be different for each image on the topic, e.g. depth and color archetypes could be mixed here!
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

        if let Some(frame_ids) = spatial_frame_ids {
            chunk_components.extend(
                CoordinateFrame::update_fields()
                    .with_many_frame(frame_ids.image_plane_frame_ids)
                    .columns_of_unit_batches()?,
            );
        }

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            chunk_components.into_iter().collect(),
        )?])
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::EntityPath;
    use re_log_types::TimeType;

    use super::*;

    fn test_ctx() -> ParserContext {
        ParserContext::new(
            EntityPath::from("/tests/image"),
            "/tests/image",
            TimeType::TimestampNs,
        )
    }

    fn install_warn_logger() -> re_log::Receiver<re_log::LogMsg> {
        re_log::setup_logging();
        let (logger, log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Warn);
        re_log::add_boxed_logger(Box::new(logger)).expect("Failed to add logger");
        log_rx
    }

    #[track_caller]
    fn expect_single_matching_warning(
        log_rx: &re_log::Receiver<re_log::LogMsg>,
        expected_substring: &str,
    ) {
        let matching_logs = std::iter::from_fn(|| log_rx.try_recv().ok())
            .filter(|log| log.level == re_log::Level::Warn && log.msg.contains(expected_substring))
            .collect::<Vec<_>>();
        assert_eq!(
            matching_logs.len(),
            1,
            "Expected exactly one matching warning containing {expected_substring:?}, got {matching_logs:?}"
        );
    }

    #[test]
    fn omits_coordinate_frame_when_any_image_frame_id_is_missing() {
        let log_rx = install_warn_logger();

        let parser = ImageMessageParser {
            blobs: vec![vec![255, 0, 0], vec![0, 255, 0]],
            image_formats: vec![ImageFormat::rgb8([1, 1]), ImageFormat::rgb8([1, 1])],
            is_depth_image: false,
            frame_ids: vec!["camera".to_owned(), "".to_owned()],
        };

        let chunks = Box::new(parser).finalize(test_ctx()).unwrap();
        let chunk = chunks.first().unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(
            !chunk
                .components()
                .contains_component(CoordinateFrame::descriptor_frame().component)
        );
        expect_single_matching_warning(&log_rx, "plain 2D image data");
    }

    #[test]
    fn keeps_coordinate_frame_for_valid_image_frame_ids() {
        let parser = ImageMessageParser {
            blobs: vec![vec![255, 0, 0]],
            image_formats: vec![ImageFormat::rgb8([1, 1])],
            is_depth_image: false,
            frame_ids: vec!["camera".to_owned()],
        };

        let chunks = Box::new(parser).finalize(test_ctx()).unwrap();
        let chunk = chunks.first().unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(
            chunk
                .components()
                .contains_component(CoordinateFrame::descriptor_frame().component)
        );
    }
}

/// A raw image encoding string, as used by ROS and Foxglove.
///
/// OpenCV-style single-channel encodings (`8UC1`, `16UC1`, etc.) are treated as depth formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumString, strum::VariantNames)]
pub enum ImageEncoding {
    #[strum(to_string = "rgb8")]
    Rgb8,
    #[strum(to_string = "rgba8")]
    Rgba8,
    #[strum(to_string = "rgb16")]
    Rgb16,
    #[strum(to_string = "rgba16")]
    Rgba16,
    #[strum(to_string = "bgr8")]
    Bgr8,
    #[strum(to_string = "bgra8")]
    Bgra8,
    #[strum(to_string = "bgr16")]
    Bgr16,
    #[strum(to_string = "bgra16")]
    Bgra16,
    #[strum(to_string = "mono8")]
    Mono8,
    #[strum(to_string = "mono16")]
    Mono16,
    #[strum(to_string = "yuyv", serialize = "yuv422_yuy2")]
    Yuyv,
    #[strum(to_string = "nv12")]
    Nv12,
    // OpenCV-style single-channel (depth) formats
    #[strum(to_string = "8UC1")]
    Cv8UC1,
    #[strum(to_string = "8UC3")]
    Cv8UC3,
    #[strum(to_string = "8SC1")]
    Cv8SC1,
    #[strum(to_string = "16UC1")]
    Cv16UC1,
    #[strum(to_string = "16SC1")]
    Cv16SC1,
    #[strum(to_string = "32SC1")]
    Cv32SC1,
    #[strum(to_string = "32FC1")]
    Cv32FC1,
    #[strum(to_string = "64FC1")]
    Cv64FC1,
}

impl ImageEncoding {
    /// All encoding name strings accepted by [`std::str::FromStr`].
    pub const NAMES: &[&str] = <Self as strum::VariantNames>::VARIANTS;

    /// Returns `true` for OpenCV-style single-channel encodings (e.g. `8UC1`, `16UC1`, `32FC1`).
    pub fn is_single_channel(self) -> bool {
        matches!(
            self,
            Self::Cv8UC1
                | Self::Cv8SC1
                | Self::Cv16UC1
                | Self::Cv16SC1
                | Self::Cv32SC1
                | Self::Cv32FC1
                | Self::Cv64FC1
                | Self::Mono8
                | Self::Mono16
        )
    }

    /// Converts this encoding into a Rerun [`ImageFormat`] for the given dimensions.
    pub fn to_image_format(self, dimensions: [u32; 2]) -> ImageFormat {
        match self {
            Self::Rgb8 => ImageFormat::rgb8(dimensions),
            Self::Rgba8 => ImageFormat::rgba8(dimensions),
            Self::Rgb16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::RGB, ChannelDatatype::U16)
            }
            Self::Rgba16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::RGBA, ChannelDatatype::U16)
            }
            Self::Bgr8 | Self::Cv8UC3 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGR, ChannelDatatype::U8)
            }
            Self::Bgra8 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGRA, ChannelDatatype::U8)
            }
            Self::Bgr16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGR, ChannelDatatype::U16)
            }
            Self::Bgra16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::BGRA, ChannelDatatype::U16)
            }
            Self::Mono8 => {
                ImageFormat::from_color_model(dimensions, ColorModel::L, ChannelDatatype::U8)
            }
            Self::Mono16 => {
                ImageFormat::from_color_model(dimensions, ColorModel::L, ChannelDatatype::U16)
            }
            Self::Yuyv => ImageFormat::from_pixel_format(dimensions, PixelFormat::YUY2),
            Self::Nv12 => ImageFormat::from_pixel_format(dimensions, PixelFormat::NV12),
            Self::Cv8UC1 => ImageFormat::depth(dimensions, ChannelDatatype::U8),
            Self::Cv8SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I8),
            Self::Cv16UC1 => ImageFormat::depth(dimensions, ChannelDatatype::U16),
            Self::Cv16SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I16),
            Self::Cv32SC1 => ImageFormat::depth(dimensions, ChannelDatatype::I32),
            Self::Cv32FC1 => ImageFormat::depth(dimensions, ChannelDatatype::F32),
            Self::Cv64FC1 => ImageFormat::depth(dimensions, ChannelDatatype::F64),
        }
    }
}

/// Parses a raw image encoding string (shared by ROS and Foxglove) into an [`ImageEncoding`].
///
/// Supports common encoding strings such as `rgb8`, `mono16`, `16UC1`, `yuyv`, `nv12`, etc.
pub fn decode_image_encoding(encoding: &str) -> anyhow::Result<ImageEncoding> {
    encoding.parse().map_err(|_err| {
        anyhow::anyhow!(
            "Unsupported image encoding '{encoding}'. Supported encodings: {:?}",
            ImageEncoding::NAMES
        )
    })
}

/// Decodes a raw image encoding string (shared by ROS and Foxglove) into a Rerun [`ImageFormat`].
///
/// Supports common encoding strings such as `rgb8`, `mono16`, `16UC1`, `yuyv`, `nv12`, etc.
/// OpenCV-style single-channel encodings (`8UC1`, `16UC1`, etc.) are treated as depth formats.
pub fn decode_image_format(encoding: &str, dimensions: [u32; 2]) -> anyhow::Result<ImageFormat> {
    Ok(decode_image_encoding(encoding)?.to_image_format(dimensions))
}
