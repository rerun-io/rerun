---
title: Query video streams
order: 70
---

Video streams provide the best compression ratio for camera feeds, but require special handling when querying data back from the dataplatform.
For more details about the different video types we support see our [video reference](../../concepts/logging-and-ingestion/video.md).

This guide focuses on querying [`VideoStream`](../../reference/types/archetypes/video_stream.md) data from the Rerun dataplatform,
including how to decode individual frames and how to export entire streams to MP4 files.

The dependencies in this example require `rerun-sdk[all]` and `av` for video decoding.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your cloud instance.

snippet: howto/query_videos[setup]

## Understanding video stream data

Video streams are logged using the [`VideoStream`](../../reference/types/archetypes/video_stream.md) archetype,
which stores encoded video samples (frames) along with codec information.

Key columns you'll work with:

- `VideoStream:codec` - The video codec used (e.g., H.264)
- `VideoStream:sample` - The encoded video frame data (in Annex B format for H.264)

## Checking the video codec

Before processing video data, verify the codec matches what you expect:

snippet: howto/query_videos[check_codec]

## Decoding a specific frame

Unlike raw images, video frames are encoded using inter-frame compression.
To decode a specific frame, you must decode from the beginning of the stream (or from the most recent keyframe) and iterate forward.
`av` handles keyframe detection internally during decoding.

snippet: howto/query_videos[decode_frame]

## Efficient random access with keyframe information

The example above queries all samples from the start of the stream, which can be inefficient for long videos.
For better performance with random access, you can add keyframe information as a layer.

### Adding keyframe information as a layer

You can analyze your video data once to identify keyframes and register them as a separate layer:

snippet: howto/query_video_keyframes[add_keyframe_column]

This preprocessing approach:

- Decodes the video once to detect which packets are keyframes using `packet.is_keyframe`
- Creates sparse data containing only keyframe timestamps
- Writes the keyframe data to a separate RRD file
- Registers it as a layer on the dataset

Once registered, the layer data appears as additional columns when querying the dataset (see [catalog object model](../../concepts/query-and-transform/catalog-object-model.md#datasets) for details on datasets and layers).

### Querying with keyframe information

With the keyframe layer registered, you can query only the samples between the nearest keyframe and your target frame,
significantly reducing the amount of data to fetch and decode:

snippet: howto/query_video_keyframes[query_with_keyframes]

This approach is especially beneficial for:

- Long video sequences where decoding from the start is expensive
- Random access patterns where you need to jump to arbitrary frames
- High-resolution video where bandwidth and decode time are significant
- Interactive applications that need to seek to specific timestamps

## Exporting to MP4 (remuxing)

You can export video stream data to an MP4 file without re-encoding.
This is called "remuxing", the encoded samples are simply repackaged into a container format.

snippet: howto/query_videos[export_mp4]

## Important considerations

### Keyframe handling

Video streams often use inter-frame compression where most frames only store the difference from previous frames.

`av` handles keyframe detection internally, but for efficient random access to specific frames,
you may want to log keyframe indicators separately at recording time.

### Timestamp handling

Video timestamps in Rerun are typically stored in nanoseconds.
When using PyAV for decoding or muxing, ensure you set the correct `time_base` (typically `Fraction(1, 1_000_000_000)`).

### B-frames

Currently, Rerun's [`VideoStream`](../../reference/types/archetypes/video_stream.md) does not support B-frames,
so `dts` (decode timestamp) equals `pts` (presentation timestamp).
