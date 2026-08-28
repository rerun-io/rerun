# re_gpu_video

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_gpu_video.svg)](https://crates.io/crates/re_gpu_video?speculative-link)
[![Documentation](https://docs.rs/re_gpu_video/badge.svg)](https://docs.rs/re_gpu_video?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Hardware video decoding to `wgpu` textures.

|            | Vulkan | Video Toolbox |
|:----------:|:------:|:-------------:|
| H.264/AVC  | ✅     | ❌            |
| H.265/HEVC | ❌     | ❌            |
| AV1        | ❌     | ❌            |

- ✅ - should work
- ❌ - not supported yet, but support is planned
