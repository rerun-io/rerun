use anyhow::bail;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_sdk_types::archetypes::{CoordinateFrame, EncodedDepthImage, EncodedImage, VideoStream};
use re_sdk_types::components::{MediaType, VideoCodec};

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::sensor_msgs;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::parsers::ros2msg::util::suffix_image_plane_frame_ids;
use crate::util::TimestampCell;

pub struct CompressedImageMessageParser {
    blobs: Vec<Vec<u8>>,
    mode: ParsedPayloadKind,
    frame_ids: Vec<String>,
}

impl Ros1MessageParser for CompressedImageMessageParser {
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
        let mut reader = Ros1Reader::new(&msg.data);
        let image = sensor_msgs::CompressedImage::read(&mut reader)?;
        reader.finish()?;

        ctx.add_timestamp_cell(TimestampCell::from_nanos_ros1(
            image.header.stamp.as_nanos(),
            ctx.time_type(),
        ));
        self.frame_ids.push(image.header.frame_id);

        if is_rvl(&image.format) {
            self.ensure_mode(ParsedPayloadKind::DepthRvl)?;
        } else if image.format.eq_ignore_ascii_case("h264") {
            self.ensure_mode(ParsedPayloadKind::H264)?;
        } else {
            self.ensure_mode(ParsedPayloadKind::Encoded)?;
        }

        self.blobs.push(image.data);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            blobs,
            mode,
            frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let mut components: Vec<_> = match mode {
            ParsedPayloadKind::DepthRvl => EncodedDepthImage::update_fields()
                .with_many_blob(blobs)
                .with_many_media_type(std::iter::repeat_n(MediaType::rvl(), frame_ids.len()))
                .columns_of_unit_batches()?
                .collect(),
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

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(suffix_image_plane_frame_ids(frame_ids))
                .columns_of_unit_batches()?,
        );

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            components.into_iter().collect(),
        )?;

        if mode == ParsedPayloadKind::H264 {
            let codec_chunk = Chunk::builder(entity_path)
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
    !encoding.trim().is_empty()
        && remainder
            .trim()
            .to_ascii_lowercase()
            .contains("compresseddepth")
        && remainder.trim().to_ascii_lowercase().contains("rvl")
}
