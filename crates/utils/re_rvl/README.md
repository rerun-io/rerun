# re_rvl

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_rvl.svg)](https://crates.io/crates/re_rvl)
[![Documentation](https://docs.rs/re_rvl/badge.svg)](https://docs.rs/re_rvl)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Codecs and helpers for depth compression formats, with a focus on RVL (Run length encoding and Variable Length encoding schemes).
Includes utilities to parse `compressedDepth` metadata as well as decode RVL streams into either disparity (`u16`) or metric depth (`f32`) buffers.
