# mcap_protobuf Example Notes

## Overview

This example demonstrates how to read custom Protobuf messages from [MCAP](https://mcap.dev/) files and convert them to Rerun data using the **Lenses API** (currently unstable).

## Supported Foxglove Message Types

- `foxglove.CameraCalibration` - Camera intrinsics
- `foxglove.CompressedImage` - Encoded images
- `foxglove.CompressedVideo` - Video streams
- `foxglove.FrameTransforms` - Coordinate frame transforms
- `foxglove.PoseInFrame` - Single pose
- `foxglove.PosesInFrame` - Multiple poses

## Key Components

### Arrow Combinators (`re_arrow_combinators`)

Used to transform Arrow data structures:

- `StructToFixedList` - Convert struct fields to fixed-size lists
- `PrimitiveCast` - Cast between primitive types (e.g., f64 to f32)
- `RowMajorToColumnMajor` - Convert matrix storage order
- `ListToFixedSizeList` - Convert variable-size to fixed-size lists
- `MapList` / `MapFixedSizeList` - Apply transformations to list elements

### Lenses API

Defines mappings from input MCAP columns to Rerun components via `Op` pipelines:

- `Op::access_field()` - Navigate into struct fields
- `Op::flatten()` - Flatten nested lists
- `Op::func()` - Apply custom transformation functions
- `Op::time_spec_to_nanos()` - Convert timestamps
- `Op::binary_to_list_uint8()` - Convert binary data to uint8 lists
- `Op::string_to_video_codec()` - Parse video codec strings
- `Op::constant()` - Output constant values

### LensesSink

Wraps a `GrpcSink` and applies all defined lenses. Uses `OutputMode::ForwardUnmatched` to pass through unrecognized data.

## Running

```bash
cargo run -p mcap_protobuf /path/to/some.mcap
```

### CLI Options

- `--epoch unix` (default) - Interpret timestamps as UNIX epoch (1970-01-01T00:00:00Z)
- `--epoch custom` - Treat timestamps as duration since an unknown epoch

## TODOs in the Code

- Add support for `tf`-style transforms
- Handle `format` column for CompressedImage (currently skipped because PNG contents aren't valid MIME types)
- Add `frame_id` support for video streams
- Complete FrameTransforms lens (missing rotation)
- Set `child_frame` of Pinhole and matching CoordinateFrame for images

## Dependencies

- `rerun` - Main Rerun SDK with clap feature
- `arrow` - Apache Arrow for data manipulation
- `re_arrow_combinators` - Arrow transformation utilities
- `re_log_types` - Rerun log type definitions
- `clap` - CLI argument parsing
- `anyhow` - Error handling
