---
title: Removed deprecated C++ asset, image, and video helpers
hidden: true
type: breaking
---

Rerun 0.44 removes the `Asset3D`, `AssetVideo`, and `VideoTimestamp` helpers deprecated in 0.23 and `Image::from_greyscale8`, deprecated in 0.24.

On `Asset3D`, use `from_file_path` and `from_file_contents` instead of `from_file` and `from_bytes`.
Use `AssetVideo::read_frame_timestamps_nanos` instead of `read_frame_timestamps_ns`, and `Image::from_grayscale8` instead of `from_greyscale8`.
On `VideoTimestamp`, replace `from_seconds`, `from_milliseconds`, and `from_nanoseconds` with `from_secs`, `from_millis`, and `from_nanos`.
