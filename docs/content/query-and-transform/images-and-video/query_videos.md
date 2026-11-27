---
title: Query video streams
order: 200
description: Query video streams from a catalog server and decode individual frames
---

Video streams use inter-frame encoding: most frames store only the changes from earlier frames.
Decoding a single frame therefore needs the preceding frames back to the last keyframe, which takes special handling when querying data back from a catalog server.
For more details about the different video types we support see our [video reference](../../log/media/video.md).

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

This guide focuses on querying [`VideoStream`](../../reference/types/archetypes/video_stream.md) data from a catalog server,
including how to decode individual frames and how to export entire streams to MP4 files.

The dependencies in this example require `rerun-sdk[all]` and `av` for video decoding.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your catalog server, such as Rerun Hub.

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

To decode a specific frame, you must decode from the beginning of the stream (or from the most recent keyframe) and iterate forward.
`av` handles keyframe detection internally during decoding.

snippet: howto/query_videos[decode_frame]

## Efficient random access with keyframe information

The example above queries all samples from the start of the stream, which can be inefficient for long videos.
For better performance with random access, add keyframe information to the stream.

The simplest way is to run [`rerun rrd optimize`](../../reference/cli.md#rerun-rrd-optimize) on the recordings before registering them: it adds the `VideoStream:is_keyframe` column, derived from the encoded samples.

### Adding keyframe information as a layer

If you cannot rewrite the recordings, analyze the video once to identify keyframes and register them as a separate [layer](../../catalog/layers.md):

snippet: howto/query_video_keyframes[add_keyframe_column]

This preprocessing approach:

- Decodes the video once to detect which packets are keyframes using `packet.is_keyframe`
- Creates sparse data containing only keyframe timestamps
- Writes the keyframe data to a separate RRD file
- Registers it as a layer on the dataset

Once registered, the layer data appears as additional columns when querying the dataset (see [catalog object model](../../catalog/catalog-object-model.md#datasets) for details on datasets and layers).

### Querying with keyframe information

With the keyframe layer registered, you can query only the samples between the nearest keyframe and your target frame,
significantly reducing the amount of data to fetch and decode:

snippet: howto/query_video_keyframes[query_with_keyframes]

## Exporting to MP4 (remuxing)

You can export video stream data to an MP4 file without re-encoding.
This is called "remuxing", the encoded samples are simply repackaged into a container format.

snippet: howto/query_videos[export_mp4]

## Important considerations

### Keyframe handling

For efficient random access to specific frames, you need keyframe indicators: log them at recording time, or add them with `rerun rrd optimize`.
[`VideoStream`](../../reference/types/archetypes/video_stream.md) has an optional `is_keyframe` component for this, which is queryable as the `VideoStream:is_keyframe` column.
`rerun.experimental.Mp4Reader` logs it when it imports an MP4 file as a video stream, and its `info()` method lists a file's keyframes from the container metadata, without decoding the video.

### Timestamp handling

Video timestamps in Rerun are typically stored in nanoseconds.
When using PyAV for decoding or muxing, ensure you set the correct `time_base` (typically `Fraction(1, 1_000_000_000)`).

### B-frames

Currently, Rerun's [`VideoStream`](../../reference/types/archetypes/video_stream.md) does not support B-frames,
so `dts` (decode timestamp) equals `pts` (presentation timestamp).
