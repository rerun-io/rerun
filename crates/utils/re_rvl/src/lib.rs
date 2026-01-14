//! RVL codec helpers (Run length encoding and Variable Length encoding schemes).
//!
//! For a complete reference of the format see:
//! <https://www.microsoft.com/en-us/research/wp-content/uploads/2018/09/p100-wilson.pdf>

use std::mem::size_of;

use byteorder::{ByteOrder as _, LittleEndian};
use thiserror::Error;

const CONFIG_HEADER_SIZE: usize = size_of::<i32>() + size_of::<[f32; 2]>();
const RESOLUTION_HEADER_SIZE: usize = size_of::<[u32; 2]>();

/// Metadata extracted from a ROS2 `compressedDepth` RVL payload.
///
/// We haven't found any other documentation on this other than the implementation itself.
/// <https://github.com/ros-perception/image_transport_plugins/blob/jazzy/compressed_depth_image_transport/src/codec.cpp>
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RosRvlMetadata {
    pub width: u32,
    pub height: u32,
    pub depth_quant_a: f32,
    pub depth_quant_b: f32,
    payload_offset: usize,
    num_pixels: usize,
}

impl RosRvlMetadata {
    #[inline]
    pub fn payload<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], RvlDecodeError> {
        bytes
            .get(self.payload_offset..)
            .ok_or(RvlDecodeError::UnexpectedEof)
    }

    #[inline]
    pub fn num_pixels(&self) -> usize {
        self.num_pixels
    }

    /// Parses RVL metadata from the start of a RVL payload.
    pub fn parse(data: &[u8]) -> Result<Self, RvlDecodeError> {
        if data.len() <= CONFIG_HEADER_SIZE {
            return Err(RvlDecodeError::MissingHeader);
        }

        let config = &data[..CONFIG_HEADER_SIZE];
        let quant_offset = size_of::<i32>();
        let depth_quant_a = LittleEndian::read_f32(&config[quant_offset..quant_offset + 4]);
        let depth_quant_b = LittleEndian::read_f32(&config[quant_offset + 4..quant_offset + 8]);

        if data.len() < CONFIG_HEADER_SIZE + RESOLUTION_HEADER_SIZE {
            return Err(RvlDecodeError::MissingResolution);
        }
        let resolution_offset = CONFIG_HEADER_SIZE;
        let width = LittleEndian::read_u32(&data[resolution_offset..resolution_offset + 4]);
        let height = LittleEndian::read_u32(&data[resolution_offset + 4..resolution_offset + 8]);
        if width == 0 || height == 0 {
            return Err(RvlDecodeError::ZeroResolution);
        }

        let payload_offset = CONFIG_HEADER_SIZE + RESOLUTION_HEADER_SIZE;
        let num_pixels = (width as u64)
            .checked_mul(height as u64)
            .ok_or(RvlDecodeError::ResolutionOverflow)? as usize;

        if data.len() < payload_offset {
            return Err(RvlDecodeError::PayloadLengthMismatch { width, height });
        }

        Ok(Self {
            width,
            height,
            depth_quant_a,
            depth_quant_b,
            payload_offset,
            num_pixels,
        })
    }
}

#[derive(Debug, Error)]
pub enum RvlDecodeError {
    #[error("compressed depth payload missing RVL header")]
    MissingHeader,

    #[error("RVL payload missing resolution header")]
    MissingResolution,

    #[error("RVL payload reports zero resolution")]
    ZeroResolution,

    #[error("RVL image resolution would overflow")]
    ResolutionOverflow,

    #[error("RVL payload shorter than expected for resolution {width}x{height}")]
    PayloadLengthMismatch { width: u32, height: u32 },

    #[error("RVL stream encodes more zero pixels than expected")]
    TooManyZeros,

    #[error("RVL stream encodes more non-zero pixels than expected")]
    TooManyNonZeros,

    #[error("RVL stream encoded an empty run (zero zeros and zero non-zeros)")]
    NoProgress,

    #[error("RVL decoded value {value} does not fit into u16")]
    ValueOutOfRange { value: i32 },

    #[error("RVL stream ended unexpectedly")]
    UnexpectedEof,

    #[error("RVL VLE value overflowed")]
    ValueOverflow,
}

fn decode_rvl_without_quantization(
    data: &[u8],
    metadata: &RosRvlMetadata,
) -> Result<Vec<u16>, RvlDecodeError> {
    let payload = metadata.payload(data)?;
    let mut disparity = vec![0u16; metadata.num_pixels()];
    let mut decoder = RvlDecoder::new(payload);
    decoder.decode_into(&mut disparity)?;
    Ok(disparity)
}

pub fn decode_rvl_with_quantization(
    data: &[u8],
    metadata: &RosRvlMetadata,
) -> Result<Vec<f32>, RvlDecodeError> {
    let disparity = decode_rvl_without_quantization(data, metadata)?;
    let mut depth = Vec::with_capacity(disparity.len());

    // ROS2's compressed_depth_image_transport sets inverse depth quantization parameters only for 32FC1 images.
    // For 16UC1, depth_quant_a/b are zero and zeros in the disparity map represent zero depth.
    // https://github.com/ros-perception/image_transport_plugins/blob/8aa39fe13a812273066bbef9b3c330508bd21618/compressed_depth_image_transport/src/codec.cpp#L263
    let has_quantization = metadata.depth_quant_a != 0.0;

    if has_quantization {
        for value in disparity {
            if value == 0 {
                depth.push(f32::NAN);
            } else {
                let quantized = value as f32;
                depth.push(metadata.depth_quant_a / (quantized - metadata.depth_quant_b));
            }
        }
    } else {
        depth.extend(disparity.into_iter().map(|value| value as f32));
    }
    Ok(depth)
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

    #[expect(clippy::cast_possible_wrap)]
    fn decode_into(&mut self, output: &mut [u16]) -> Result<(), RvlDecodeError> {
        let mut remaining = output.len();
        let mut write_index = 0;
        let mut previous: i32 = 0;

        while remaining > 0 {
            let zeros = self.decode_vle()? as usize;
            if zeros > remaining {
                return Err(RvlDecodeError::TooManyZeros);
            }
            for value in output.iter_mut().skip(write_index).take(zeros) {
                *value = 0;
            }
            write_index += zeros;
            remaining -= zeros;

            let nonzeros = self.decode_vle()? as usize;
            if nonzeros > remaining {
                return Err(RvlDecodeError::TooManyNonZeros);
            }
            if zeros == 0 && nonzeros == 0 {
                return Err(RvlDecodeError::NoProgress);
            }
            for value in output.iter_mut().skip(write_index).take(nonzeros) {
                let positive = self.decode_vle()? as i32;
                let delta = (positive >> 1) ^ -(positive & 1);
                previous = previous.wrapping_add(delta);
                if !(0..=u16::MAX as i32).contains(&previous) {
                    return Err(RvlDecodeError::ValueOutOfRange { value: previous });
                }
                *value = previous as u16;
            }
            write_index += nonzeros;
            remaining -= nonzeros;
        }

        Ok(())
    }

    fn decode_vle(&mut self) -> Result<u32, RvlDecodeError> {
        let mut value = 0u32;
        let mut shift = 0u32;

        loop {
            let nibble = self.next_nibble()?;
            value |= u32::from(nibble & 0x7) << shift;

            if nibble & 0x8 == 0 {
                break;
            }

            shift += 3;
            if shift >= 32 {
                return Err(RvlDecodeError::ValueOverflow);
            }
        }

        Ok(value)
    }

    fn next_nibble(&mut self) -> Result<u8, RvlDecodeError> {
        if self.nibbles_remaining == 0 {
            if self.offset + size_of::<u32>() > self.input.len() {
                return Err(RvlDecodeError::UnexpectedEof);
            }
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
    fn detects_metadata() {
        let disparity = [0u16, 1200, 1201, 0, 800];
        let data = build_depth_message([5, 1], &disparity, (0.0, 0.0));
        let metadata = RosRvlMetadata::parse(&data).unwrap();
        assert_eq!(metadata.width, 5);
        assert_eq!(metadata.height, 1);
    }

    #[test]
    fn decodes_rvl_u16_payload() {
        let disparity = [0u16, 1200, 1201, 0, 800];
        let data = build_depth_message([5, 1], &disparity, (0.0, 0.0));
        let metadata = RosRvlMetadata::parse(&data).unwrap();
        let decoded = decode_rvl_without_quantization(&data, &metadata).unwrap();
        assert_eq!(decoded, disparity);
    }

    #[test]
    fn allows_high_compression_u16_payload() {
        let width = 64;
        let height = 48;
        let disparity = vec![0u16; (width * height) as usize];
        let data = build_depth_message([width, height], &disparity, (0.0, 0.0));
        let metadata = RosRvlMetadata::parse(&data).unwrap();
        let decoded = decode_rvl_without_quantization(&data, &metadata).unwrap();
        assert_eq!(decoded, disparity);
    }

    #[test]
    fn decodes_rvl_f32_payload() {
        let disparity = [5u16, 0, 10];
        let depth_params = (10.0, 1.0);
        let data = build_depth_message([3, 1], &disparity, depth_params);
        let metadata = RosRvlMetadata::parse(&data).unwrap();
        let decoded = decode_rvl_with_quantization(&data, &metadata).unwrap();
        assert!((decoded[0] - 2.5).abs() < 1e-6);
        assert!(decoded[1].is_nan());
        assert!((decoded[2] - (10.0 / 9.0)).abs() < 1e-6);
    }

    #[test]
    fn decodes_rvl_with_quantization_when_parameters_zero() {
        let disparity = [0u16, 1200, 0];
        let quant_params = (0.0, 0.0);
        let data = build_depth_message([3, 1], &disparity, quant_params);
        let metadata = RosRvlMetadata::parse(&data).unwrap();
        let decoded = decode_rvl_with_quantization(&data, &metadata).unwrap();
        assert!(!decoded[0].is_nan());
        assert_eq!(decoded[0], 0.0);
        assert_eq!(decoded[1], 1200.0);
        assert_eq!(decoded[2], 0.0);
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
        // TODO(gijs, andreas): This would actually be a nice addition to the crate as well!
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
