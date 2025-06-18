<!--[metadata]
title = "Compressed Camera Video Stream"
tags = ["2D", "Image encoding", "Video", "Streaming"]
thumbnail = "https://static.rerun.io/camera_video_stream/e3cd0bba4929b766d9fc65b4c6fc70081f6b8cbc/480w.png"
thumbnail_dimensions = [480, 269]
-->

This example uses [pyAV](https://pypi.org/project/av/) to fetch and encode a video stream to H.264 video and streams it live
to the Viewer using the [`VideoStream`](https://www.rerun.io/docs/reference/types/archetypes/video_stream#speculative-link) archetype

<img src="https://static.rerun.io/camera_video_stream/e3cd0bba4929b766d9fc65b4c6fc70081f6b8cbc/480w.png">


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

Video frames are logged subsequently using 
```py
rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))
```

It's best practice to log the codec only once as it never changes, but there is little harm in
doing so for every frame:
```py
rr.log("video_stream", rr.VideoStream(codec=rr.VideoCodec.H264, sample=bytes(packet)))
```
