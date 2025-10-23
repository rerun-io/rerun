<!--[metadata]
title = "Compressed camera video stream"
tags = ["2D", "Image encoding", "Video", "Streaming"]
thumbnail = "https://static.rerun.io/camera_video_stream/b2f8f61eb62424aa942bdb5183e49246cf417e60/480w.png"
thumbnail_dimensions = [480, 300]
-->

This example uses [pyAV](https://pypi.org/project/av/) to fetch and encode a video stream to H.264 video and streams it live
to the Viewer using the [`VideoStream`](https://www.rerun.io/docs/reference/types/archetypes/video_stream) archetype

<img src="https://static.rerun.io/camera_video_stream/b2f8f61eb62424aa942bdb5183e49246cf417e60/480w.png">


## Run the code

```bash
pip install -e examples/python/camera_video_stream
python -m camera_video_stream
```

## Details

To learn more about video support in general check [the docs page on the topic](https://rerun.io/docs/reference/video).

The example first sets up a video stream on an entity called `"video_stream"` with the H264 codec.
```py
rr.log("video_stream", rr.VideoStream(codec=rr.VideoCodec.H264), static=True)
```
H264 is a very well established codec and well suited for streaming.
Note that in `setup_output_stream` we specicifially configure `pyAV` to use low latency encoding.

⚠️ Latency is expected to be well below a second, but as of writing Rerun is not well
optimized for low latency video transmission as used for teleoperations.
If you need lower latency you should consider tweaking the encoding settings further and
configure Rerun's [Micro Batching](https://rerun.io/docs/reference/sdk/micro-batching).

For each frame we have to set a new timestamp.
```py
rr.set_time("time", duration=float(packet.pts * packet.time_base))
```
The time set here is the [_presentation timestamp_](https://en.wikipedia.org/wiki/Presentation_timestamp) (PTS) of the frame.
Note that unlike with unlike with [`VideoAsset`](https://www.rerun.io/docs/reference/types/archetypes/asset_video),
there's no need to log [`VideoFrameReference`](https://www.rerun.io/docs/reference/types/archetypes/video_frame_reference),
to map the video's PTS to the Rerun timeline, since the time at which video samples
are logged directly represents the PTS.
TODO(#10090): In the presence of H.264/H.265 b-frames, separate _decode timestamps_ (DTS) are needed. This is not yet supported.

The frame data, known as a frame-`sample` since this may contain data relevant for an arbitrary number of frames in the future,
is then logged with:
```py
rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))
```

It's best practice to log the codec only once as it never changes, but there is little harm in
doing so for every frame:
```py
rr.log("video_stream", rr.VideoStream(codec=rr.VideoCodec.H264, sample=bytes(packet)))
```
