# re_depth_compression

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_depth_compression.svg)](https://crates.io/crates/re_depth_compression)
[![Documentation](https://docs.rs/re_depth_compression/badge.svg)](https://docs.rs/re_depth_compression)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Codecs and helpers for depth compression formats, with a focus on RVL (Range Image Visualization Library).
Includes utilities to parse ROS2 `compressedDepth` metadata as well as decode RVL streams into either disparity (`u16`) or metric depth (`f32`) buffers.
