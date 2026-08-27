# re_gpu_video

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_gpu_video.svg)](https://crates.io/crates/re_gpu_video)
[![Documentation](https://docs.rs/re_gpu_video/badge.svg)](https://docs.rs/re_gpu_video)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Hardware video decoding straight to `wgpu` textures, without the decoded frames ever leaving the GPU.

Two backends, chosen at runtime based on the wgpu backend of the adapter:
* Vulkan Video: any Vulkan driver exposing the video decode extensions
* VideoToolbox (macOS, not yet implemented)

Decode-only: H.264, H.265, and AV1.
