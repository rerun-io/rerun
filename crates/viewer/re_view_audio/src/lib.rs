//! Rerun audio View.
//!
//! A View that shows audio clips as time-aligned waveforms.

mod view_class;
mod visualizer_system;

#[cfg(not(target_arch = "wasm32"))]
mod playback;
mod processing;

pub use view_class::AudioView;
