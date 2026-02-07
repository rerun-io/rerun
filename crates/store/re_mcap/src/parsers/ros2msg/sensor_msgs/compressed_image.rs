use anyhow::bail;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_sdk_types::archetypes::{CoordinateFrame, EncodedDepthImage, EncodedImage, VideoStream};
use re_sdk_types::components::{MediaType, VideoCodec};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use super::super::util::suffix_image_plane_frame_ids;
use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::util::TimestampCell;

/// Plugin that parses `sensor_msgs/msg/CompressedImage` messages.
pub struct CompressedImageMessageParser {
    /// The raw image data blobs.
    ///
    /// Note: These blobs are directly moved into a `Blob`, without copying.
    blobs: Vec<Vec<u8>>,
    mode: ParsedPayloadKind,
    frame_ids: Vec<String>,
}

impl Ros2MessageParser for CompressedImageMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            blobs: Vec::with_capacity(num_rows),
            mode: ParsedPayloadKind::Unknown,
            frame_ids: Vec::with_capacity(num_rows),
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
        self.frame_ids.push(header.frame_id);

        let data = data.into_owned();

        if is_rvl(&format) {
            self.ensure_mode(ParsedPayloadKind::DepthRvl)?;
        } else if format.eq_ignore_ascii_case("h264") {
            self.ensure_mode(ParsedPayloadKind::H264)?;
        } else {
            self.ensure_mode(ParsedPayloadKind::Encoded)?;
        }

        self.blobs.push(data);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            blobs,
            mode,
            frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let mut components: Vec<_> = match mode {
            ParsedPayloadKind::DepthRvl => {
                let media_types = std::iter::repeat_n(MediaType::rvl(), blobs.len());
                EncodedDepthImage::update_fields()
                    .with_many_blob(blobs)
                    .with_many_media_type(media_types)
                    .columns_of_unit_batches()?
                    .collect()
            }
            ParsedPayloadKind::H264 => VideoStream::update_fields()
                .with_many_sample(blobs)
                .columns_of_unit_batches()?
                .collect(),

            ParsedPayloadKind::Unknown | ParsedPayloadKind::Encoded => {
                EncodedImage::update_fields()
                    .with_many_blob(blobs)
                    .columns_of_unit_batches()?
                    .collect()
            }
        };

        // We need a frame ID for the image plane. This doesn't exist in ROS,
        // so we use the camera frame ID with a suffix here (see also camera info parser).
        let image_plane_frame_ids = suffix_image_plane_frame_ids(frame_ids);
        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(image_plane_frame_ids)
                .columns_of_unit_batches()?,
        );

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            components.into_iter().collect(),
        )?;

        if mode == ParsedPayloadKind::H264 {
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
    H264,
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
                "Encountered mixed compressed image payloads on the same topic; this is not supported"
            ),
        }
    }
}

fn is_rvl(format: &str) -> bool {
    let Some((encoding, remainder)) = format.split_once(';') else {
        return false;
    };
    let encoding = encoding.trim();
    if encoding.is_empty() {
        return false;
    }

    let remainder_lower = remainder.trim().to_ascii_lowercase();
    remainder_lower.contains("compresseddepth") && remainder_lower.contains("rvl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_depth_rvl_format() {
        assert!(is_rvl("16UC1; compressedDepth RVL"));
        assert!(is_rvl("32FC1; compressedDepth RVL"));
        assert!(!is_rvl("16UC1; compressedDepth png"));
        assert!(!is_rvl("jpeg"));
    }
}
