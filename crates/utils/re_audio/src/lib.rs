//! Audio decoding and playback for the Rerun Viewer.
//!
//! With the `decoding` feature, `decode` turns an encoded audio file into an [`AudioBuffer`]
//! of interleaved `f32` PCM.
//! With the `output` feature, [`AudioPlayer`] mixes any number of such buffers and plays them
//! through the system's default output device, keeping each stream aligned with a caller-driven
//! playhead. On Linux the ALSA library is loaded at runtime, so it is not a hard dependency.

mod buffer;
mod downmix;
mod request;
mod test_sound;
mod waveform_envelope;

#[cfg(feature = "decoding")]
mod decoding;

#[cfg(feature = "output")]
mod mixer;
#[cfg(feature = "output")]
mod output;
#[cfg(feature = "output")]
mod player;

pub use buffer::{AudioBuffer, ChannelLayout, ChannelPosition};
pub use request::{AUDIBLE_SPEEDS, StreamId, StreamRequest};
pub use test_sound::test_sound;
pub use waveform_envelope::WaveformEnvelope;

#[cfg(feature = "decoding")]
pub use decoding::{AudioDecodeError, decode};

#[cfg(feature = "output")]
pub use output::OutputError;
#[cfg(feature = "output")]
pub use player::AudioPlayer;
