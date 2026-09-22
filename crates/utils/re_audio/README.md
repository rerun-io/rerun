# re_audio

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_audio.svg?speculative-link)](https://crates.io/crates/re_audio?speculative-link)
[![Documentation](https://docs.rs/re_audio/badge.svg?speculative-link)](https://docs.rs/re_audio?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Decodes audio files (WAV, AAC, MP3, FLAC, Ogg Vorbis, M4A) into PCM and computes waveform envelopes for efficient visualization.

Decoding uses [`symphonia`](https://github.com/pdeljanov/Symphonia): it is pure Rust (so it also builds for the web), covers all the formats above in one crate, and is widely used across the Rust audio ecosystem.
