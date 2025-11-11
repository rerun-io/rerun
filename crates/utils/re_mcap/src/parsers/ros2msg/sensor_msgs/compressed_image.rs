use std::mem::size_of;

use super::super::definitions::sensor_msgs;
use anyhow::{Context as _, bail, ensure};
use byteorder::{ByteOrder, LittleEndian};
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_types::{
    archetypes::{DepthImage, EncodedImage, VideoStream},
    components::VideoCodec,
    datatypes::{ChannelDatatype, ImageFormat},
};

use super::super::Ros2MessageParser;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
};
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

        if let Some(depth_encoding) = depth_rvl_encoding(&format) {
            self.ensure_mode(ParsedPayloadKind::DepthRvl)?;

            let (decoded, image_format) = decode_rvl_depth_image(depth_encoding, &data)
                .with_context(|| {
                    format!("Failed to decode RVL compressed depth image with format '{format}'")
                })?;

            self.image_formats.push(image_format);
            self.blobs.push(decoded);
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
            ParsedPayloadKind::DepthRvl => DepthImage::update_fields()
                .with_many_buffer(blobs)
                .with_many_format(image_formats)
                .columns_of_unit_batches()?
                .collect(),
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

fn depth_rvl_encoding(format: &str) -> Option<&str> {
    let (encoding, remainder) = format.split_once(';')?;
    let encoding = encoding.trim();
    if encoding.is_empty() {
        return None;
    }

    let remainder_lower = remainder.trim().to_ascii_lowercase();
    if remainder_lower.contains("compresseddepth") && remainder_lower.contains("rvl") {
        Some(encoding)
    } else {
        None
    }
}

fn decode_rvl_depth_image(
    encoding: &str,
    message_data: &[u8],
) -> anyhow::Result<(Vec<u8>, ImageFormat)> {
    const CONFIG_HEADER_SIZE: usize = size_of::<i32>() + size_of::<[f32; 2]>();
    ensure!(
        message_data.len() > CONFIG_HEADER_SIZE,
        "Compressed depth payload missing RVL header"
    );

    let config = parse_config_header(&message_data[..CONFIG_HEADER_SIZE])?;
    let payload = &message_data[CONFIG_HEADER_SIZE..];
    ensure!(
        payload.len() >= size_of::<[u32; 2]>(),
        "RVL payload missing resolution header"
    );

    let width = LittleEndian::read_u32(&payload[0..4]);
    let height = LittleEndian::read_u32(&payload[4..8]);
    ensure!(
        width > 0 && height > 0,
        "RVL payload reports zero resolution"
    );

    let num_pixels = (width as u64)
        .checked_mul(height as u64)
        .context("RVL image resolution would overflow")?;
    ensure!(num_pixels <= i32::MAX as u64, "RVL image too large");
    let encoding_upper = encoding.trim().to_ascii_uppercase();
    if encoding_upper.as_str() == "32FC1" {
        ensure!(
            num_pixels <= (payload.len() as u64) * 5,
            "RVL payload size is inconsistent with reported resolution {width}x{height}"
        );
    }

    let mut disparity = vec![0u16; num_pixels as usize];
    let mut decoder = RvlDecoder::new(&payload[8..]);
    decoder.decode_into(&mut disparity)?;

    let dimensions = [width, height];
    match encoding_upper.as_str() {
        "16UC1" => {
            let mut buffer = Vec::with_capacity(disparity.len() * size_of::<u16>());
            for value in disparity {
                buffer.extend_from_slice(&value.to_le_bytes());
            }
            let format = ImageFormat::depth(dimensions, ChannelDatatype::U16);
            Ok((buffer, format))
        }
        "32FC1" => {
            let mut buffer = Vec::with_capacity(disparity.len() * size_of::<f32>());
            for value in disparity {
                let depth = if value == 0 {
                    f32::NAN
                } else {
                    let quantized = value as f32;
                    config.depth_quant_a / (quantized - config.depth_quant_b)
                };
                buffer.extend_from_slice(&depth.to_le_bytes());
            }
            let format = ImageFormat::depth(dimensions, ChannelDatatype::F32);
            Ok((buffer, format))
        }
        other => bail!("Unsupported RVL depth encoding '{other}'"),
    }
}

struct CompressionConfig {
    depth_quant_a: f32,
    depth_quant_b: f32,
}

fn parse_config_header(bytes: &[u8]) -> anyhow::Result<CompressionConfig> {
    ensure!(
        bytes.len() >= size_of::<i32>() + size_of::<[f32; 2]>(),
        "Config header too short"
    );
    Ok(CompressionConfig {
        depth_quant_a: LittleEndian::read_f32(&bytes[size_of::<i32>()..size_of::<i32>() + 4]),
        depth_quant_b: LittleEndian::read_f32(&bytes[size_of::<i32>() + 4..size_of::<i32>() + 8]),
    })
}

struct RvlDecoder<'a> {
    input: &'a [u8],
    offset: usize,
    word: u32,
    nibbles_remaining: u8,
}

impl<'a> RvlDecoder<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            offset: 0,
            word: 0,
            nibbles_remaining: 0,
        }
    }

    fn decode_into(&mut self, output: &mut [u16]) -> anyhow::Result<()> {
        let mut remaining = output.len();
        let mut write_index = 0;
        let mut previous: i32 = 0;

        while remaining > 0 {
            let zeros = self.decode_vle()? as usize;
            ensure!(
                zeros <= remaining,
                "RVL stream encodes more zero pixels than expected"
            );

            for value in output.iter_mut().skip(write_index).take(zeros) {
                *value = 0;
            }
            write_index += zeros;
            remaining -= zeros;

            let nonzeros = self.decode_vle()? as usize;
            ensure!(
                nonzeros <= remaining,
                "RVL stream encodes more non-zero pixels than expected"
            );

            for value in output.iter_mut().skip(write_index).take(nonzeros) {
                let positive = self.decode_vle()? as i32;
                let delta = (positive >> 1) ^ -(positive & 1);
                previous = previous.wrapping_add(delta);
                ensure!(
                    (0..=u16::MAX as i32).contains(&previous),
                    "RVL decoded value {previous} does not fit into u16"
                );
                *value = previous as u16;
            }

            write_index += nonzeros;
            remaining -= nonzeros;
        }

        Ok(())
    }

    fn decode_vle(&mut self) -> anyhow::Result<u32> {
        let mut value = 0u32;
        let mut shift = 0u32;

        loop {
            let nibble = self.next_nibble()?;
            value |= u32::from(nibble & 0x7) << shift;

            if nibble & 0x8 == 0 {
                break;
            }

            shift += 3;
            ensure!(shift < 32, "RVL VLE value overflowed");
        }

        Ok(value)
    }

    fn next_nibble(&mut self) -> anyhow::Result<u8> {
        if self.nibbles_remaining == 0 {
            ensure!(
                self.offset + size_of::<u32>() <= self.input.len(),
                "RVL stream ended unexpectedly"
            );
            self.word =
                LittleEndian::read_u32(&self.input[self.offset..self.offset + size_of::<u32>()]);
            self.offset += size_of::<u32>();
            self.nibbles_remaining = 8;
        }

        let nibble = ((self.word & 0xF000_0000) >> 28) as u8;
        self.word <<= 4;
        self.nibbles_remaining -= 1;
        Ok(nibble)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_depth_rvl_format() {
        assert_eq!(
            depth_rvl_encoding("16UC1; compressedDepth RVL").unwrap(),
            "16UC1"
        );
        assert!(depth_rvl_encoding("16UC1; compressedDepth png").is_none());
        assert!(depth_rvl_encoding("jpeg").is_none());
    }

    #[test]
    fn decodes_rvl_u16_payload() {
        let disparity = [0u16, 1200, 1201, 0, 800];
        let data = build_depth_message([5, 1], &disparity, (0.0, 0.0));

        let (buffer, format) = decode_rvl_depth_image("16UC1", &data).unwrap();
        assert_eq!(format, ImageFormat::depth([5, 1], ChannelDatatype::U16));

        let decoded: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(decoded, disparity);
    }

    #[test]
    fn allows_high_compression_u16_payload() {
        let width = 64;
        let height = 48;
        let disparity = vec![0u16; (width * height) as usize];
        let data = build_depth_message([width, height], &disparity, (0.0, 0.0));

        let (buffer, format) = decode_rvl_depth_image("16UC1", &data).unwrap();
        assert_eq!(
            format,
            ImageFormat::depth([width, height], ChannelDatatype::U16)
        );
        assert_eq!(buffer.len(), disparity.len() * size_of::<u16>());
    }

    #[test]
    fn decodes_rvl_f32_payload() {
        let disparity = [5u16, 0, 10];
        let depth_params = (10.0, 1.0);
        let data = build_depth_message([3, 1], &disparity, depth_params);

        let (buffer, format) = decode_rvl_depth_image("32FC1", &data).unwrap();
        assert_eq!(format, ImageFormat::depth([3, 1], ChannelDatatype::F32));

        let decoded: Vec<f32> = buffer
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        assert!((decoded[0] - 2.5).abs() < 1e-6);
        assert!(decoded[1].is_nan());
        assert!((decoded[2] - (10.0 / 9.0)).abs() < 1e-6);
    }

    fn build_depth_message(
        dimensions: [u32; 2],
        disparity: &[u16],
        depth_params: (f32, f32),
    ) -> Vec<u8> {
        let expected_len = dimensions[0] as usize * dimensions[1] as usize;
        assert_eq!(
            expected_len,
            disparity.len(),
            "disparity length must match resolution"
        );
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&depth_params.0.to_le_bytes());
        bytes.extend_from_slice(&depth_params.1.to_le_bytes());
        bytes.extend_from_slice(&dimensions[0].to_le_bytes());
        bytes.extend_from_slice(&dimensions[1].to_le_bytes());
        let compressed = encode_rvl(disparity);
        bytes.extend_from_slice(&compressed);
        bytes
    }

    fn encode_rvl(values: &[u16]) -> Vec<u8> {
        struct Encoder {
            buffer: Vec<u8>,
            word: u32,
            nibbles_written: u8,
        }

        impl Encoder {
            fn new() -> Self {
                Self {
                    buffer: Vec::new(),
                    word: 0,
                    nibbles_written: 0,
                }
            }

            fn encode_vle(&mut self, mut value: u32) {
                loop {
                    let mut nibble = (value & 0x7) as u8;
                    value >>= 3;
                    if value != 0 {
                        nibble |= 0x8;
                    }
                    self.push_nibble(nibble);
                    if value == 0 {
                        break;
                    }
                }
            }

            fn push_nibble(&mut self, nibble: u8) {
                self.word = (self.word << 4) | u32::from(nibble);
                self.nibbles_written += 1;
                if self.nibbles_written == 8 {
                    self.buffer.extend_from_slice(&self.word.to_le_bytes());
                    self.word = 0;
                    self.nibbles_written = 0;
                }
            }

            fn finish(mut self) -> Vec<u8> {
                if self.nibbles_written > 0 {
                    let remaining = 8 - self.nibbles_written;
                    self.word <<= 4 * remaining as u32;
                    self.buffer.extend_from_slice(&self.word.to_le_bytes());
                }
                self.buffer
            }
        }

        let mut encoder = Encoder::new();
        let mut index = 0;
        let mut previous: i32 = 0;

        while index < values.len() {
            let zero_start = index;
            while index < values.len() && values[index] == 0 {
                index += 1;
            }
            let zeros = index - zero_start;
            encoder.encode_vle(zeros as u32);

            let nonzero_start = index;
            while index < values.len() && values[index] != 0 {
                index += 1;
            }
            let nonzeros = index - nonzero_start;
            encoder.encode_vle(nonzeros as u32);

            for &value in &values[nonzero_start..index] {
                let delta = (value as i32) - previous;
                let positive = ((delta << 1) ^ (delta >> 31)) as u32;
                encoder.encode_vle(positive);
                previous = value as i32;
            }
        }

        encoder.finish()
    }
}
