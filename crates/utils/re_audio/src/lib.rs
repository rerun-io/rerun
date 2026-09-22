//! Audio decoding for the Rerun Viewer.
//!
//! [`decode`] turns an encoded audio file into an [`AudioBuffer`] of interleaved `f32` PCM.

mod buffer;
mod decoding;
mod waveform_envelope;

pub use buffer::{AudioBuffer, ChannelLayout, ChannelPosition};
pub use decoding::{AudioDecodeError, decode};
pub use waveform_envelope::WaveformEnvelope;
