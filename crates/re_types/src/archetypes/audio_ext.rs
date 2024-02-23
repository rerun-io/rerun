use crate::datatypes::{TensorBuffer, TensorData, TensorDimension};

use super::Audio;

impl Audio {
    /// Create a new audio from a buffer of samples.
    ///
    /// Will panic if the buffer is compressed.
    pub fn from_mono(buffer: TensorBuffer) -> Self {
        Self::from_channels(buffer, 1)
    }

    /// Create a new audio from a buffer of interleaved samples, L,R,L,R,….
    ///
    /// Will panic if the buffer is compressed.
    pub fn from_stereo(buffer: TensorBuffer) -> Self {
        Self::from_channels(buffer, 2)
    }

    /// Create a new audio from a buffer of interleaved samples, and the number of channels.
    ///
    /// Will panic if the buffer is compressed.
    pub fn from_channels(buffer: TensorBuffer, channel_count: u64) -> Self {
        let num_samples = buffer
            .num_elements()
            .expect("Buffer must not be compressed") as u64;
        let num_frames = num_samples / channel_count;

        let shape = vec![
            TensorDimension::unnamed(num_frames),
            TensorDimension::unnamed(channel_count),
        ];
        let tensor_data = TensorData { buffer, shape };
        Self::new(tensor_data)
    }

    /// Load the contents of a `.wav` file into a new `Audio` instance.
    pub fn from_wav_bytes(wav_file: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let (header, wav_data) = wav::read(&mut std::io::Cursor::new(wav_file))?;
        re_log::trace!("WAV header: {header:?}");

        let buffer = match wav_data {
            wav::BitDepth::Eight(data) => TensorBuffer::U8(data.into()),
            wav::BitDepth::Sixteen(data) => TensorBuffer::I16(data.into()),
            // TODO: tell Rerun that this is 24-bit audio, i.e. too expect at most ±2^23, e.g. log a special "scale" attribute or something
            wav::BitDepth::TwentyFour(data) => TensorBuffer::I32(data.into()),
            wav::BitDepth::ThirtyTwoFloat(data) => TensorBuffer::F32(data.into()),
            wav::BitDepth::Empty => panic!("No audio"), // TODO: return an error
        };

        Ok(Self::from_channels(buffer, header.channel_count as u64))
    }
}
