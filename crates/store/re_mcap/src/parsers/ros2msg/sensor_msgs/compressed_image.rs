use super::super::definitions::sensor_msgs;
use anyhow::{Context as _, bail};
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_depth_compression::ros_rvl::parse_ros_rvl_metadata;
use re_sdk_types::archetypes::{EncodedDepthImage, EncodedImage, VideoStream};
use re_sdk_types::components::{ImageFormat, MediaType, VideoCodec};
use re_sdk_types::datatypes::ChannelDatatype;

use super::super::Ros2MessageParser;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::util::TimestampCell;

/// Plugin that parses `sensor_msgs/msg/CompressedImage` messages.
pub struct CompressedImageMessageParser {
    /// The raw image data blobs.
    ///
    /// Note: These blobs are directly moved into a `Blob`, without copying.
    blobs: Vec<Vec<u8>>,
    image_formats: Vec<ImageFormat>,
    mode: ParsedPayloadKind,
    is_h264: bool,
}

impl Ros2MessageParser for CompressedImageMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            blobs: Vec::with_capacity(num_rows),
            image_formats: Vec::with_capacity(num_rows),
            mode: ParsedPayloadKind::Unknown,
            is_h264: false,
        }
    }
}

impl MessageParser for CompressedImageMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let sensor_msgs::CompressedImage {
            header,
            data,
            format,
        } = cdr::try_decode_message::<sensor_msgs::CompressedImage<'_>>(&msg.data)?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        let data = data.into_owned();

        if let Some(datatype) = depth_rvl_encoding(&format) {
            self.ensure_mode(ParsedPayloadKind::DepthRvl)?;

            let metadata = parse_ros_rvl_metadata(&data).with_context(|| {
                format!("Failed to parse RVL header for compressed depth image '{format}'")
            })?;

            self.image_formats.push(ImageFormat::depth(
                [metadata.width, metadata.height],
                datatype,
            ));
            self.blobs.push(data);
        } else {
            self.ensure_mode(ParsedPayloadKind::Encoded)?;
            self.blobs.push(data);

            if format.eq_ignore_ascii_case("h264") {
                // If the format for this topic is h264 once, we assume it is h264 for all messages.
                self.is_h264 = true;
            }
        }

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            blobs,
            image_formats,
            mode,
            is_h264,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let components = match mode {
            ParsedPayloadKind::DepthRvl => {
                let media_types = std::iter::repeat_n(MediaType::rvl(), blobs.len());
                EncodedDepthImage::update_fields()
                    .with_many_blob(blobs)
                    .with_many_format(image_formats)
                    .with_many_media_type(media_types)
                    .columns_of_unit_batches()?
                    .collect()
            }
            ParsedPayloadKind::Unknown | ParsedPayloadKind::Encoded => {
                if is_h264 {
                    VideoStream::update_fields()
                        .with_many_sample(blobs)
                        .columns_of_unit_batches()?
                        .collect()
                } else {
                    EncodedImage::update_fields()
                        .with_many_blob(blobs)
                        .columns_of_unit_batches()?
                        .collect()
                }
            }
        };

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            components,
        )?;

        if matches!(
            mode,
            ParsedPayloadKind::Unknown | ParsedPayloadKind::Encoded
        ) && is_h264
        {
            // codec should be logged once per entity, as static data.
            let codec_chunk = Chunk::builder(entity_path.clone())
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &VideoStream::update_fields().with_codec(VideoCodec::H264),
                )
                .build()?;
            Ok(vec![chunk, codec_chunk])
        } else {
            Ok(vec![chunk])
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedPayloadKind {
    Unknown,
    Encoded,
    DepthRvl,
}

impl CompressedImageMessageParser {
    fn ensure_mode(&mut self, new_mode: ParsedPayloadKind) -> anyhow::Result<()> {
        match (self.mode, new_mode) {
            (ParsedPayloadKind::Unknown, mode) => {
                self.mode = mode;
                Ok(())
            }
            (mode, new_mode) if mode == new_mode => Ok(()),
            _ => bail!(
                "Encountered mixed compressed image payloads (RVL depth + encoded) on the same topic; this is not supported"
            ),
        }
    }
}

fn depth_rvl_encoding(format: &str) -> Option<ChannelDatatype> {
    let (encoding, remainder) = format.split_once(';')?;
    let encoding = encoding.trim();
    if encoding.is_empty() {
        return None;
    }

    let remainder_lower = remainder.trim().to_ascii_lowercase();
    if remainder_lower.contains("compresseddepth") && remainder_lower.contains("rvl") {
        if encoding.eq_ignore_ascii_case("32FC1") {
            Some(ChannelDatatype::F32)
        } else {
            Some(ChannelDatatype::U16)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_depth_rvl_format() {
        assert_eq!(
            depth_rvl_encoding("16UC1; compressedDepth RVL").unwrap(),
            ChannelDatatype::U16
        );
        assert_eq!(
            depth_rvl_encoding("32FC1; compressedDepth RVL").unwrap(),
            ChannelDatatype::F32
        );
        assert!(depth_rvl_encoding("16UC1; compressedDepth png").is_none());
        assert!(depth_rvl_encoding("jpeg").is_none());
    }
}
