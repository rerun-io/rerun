---
title: Video
order: 1100
---

A stream of images (like those produced by a camera) can be logged to Rerun in several different ways:

* Uncompressed, as many [`Image`](../../reference/types/archetypes/image.md)s
* Compressed as many [`EncodedImage`](../../reference/types/archetypes/encoded_image.md)s, using e.g. JPEG.
* Compressed as a single [`AssetVideo`](../../reference/types/archetypes/asset_video.md), using e.g. MP4.
* Compressed as a series of encoded video samples using [`VideoStream`](../../reference/types/archetypes/video_stream.md), using e.g. H.264 encoded frames.

These alternatives range on a scale of "simple, lossless, and big" to "complex, lossy, and small".

If you want lossless encoded images (with no compression artifacts), then you should log each video frame as `Image`.
This will use up a lot of space and bandwidth. You can also encode them as PNG and log them as `EncodedImage`,
though it should be noted that PNG encoding usually does very little for the file size of photographic images.

If you want to reduce bandwidth and storage cost, you can encode each frame as a JPEG and log it using `EncodedImage`. This can easily reduce the file sizes by almost two orders of magnitude with minimal perceptual loss.
This is also very simple to do, and the Python logging SDK has built-in support for it using [`Image.compress`](https://ref.rerun.io/docs/python/0.18.2/common/archetypes/#rerun.archetypes.Image.compress).

Finally, for the best compression ratio, you can encode the images as an encoded video.
There are two options to choose from:
* Raw video frames [`VideoStream`](../../reference/types/archetypes/video_stream.md)
* Video files using [`AssetVideo`](../../reference/types/archetypes/asset_video.md)

⚠️ Do not use compressed video if you need accurate pixel replication:
this is not only due to the obvious detail loss on encoding,
but also since the exact _display_ of the same video is not consistent across platforms and decoder versions.

## Streaming video / raw encoded video frames

The following example illustrates how to encode uncompressed video frames (represented by `numpy` arrays)
using [`pyAV`](https://github.com/PyAV-Org/PyAV) into H.264 and directly log them to Rerun using [`VideoStream`](../../reference/types/archetypes/video_stream.md).

snippet: archetypes/video_stream_synthetic

Using [`VideoStream`](../../reference/types/archetypes/video_stream.md) requires deeper knowledge of the encoding process
but unlike [`AssetVideo`](../../reference/types/archetypes/asset_video.md),
allows the Rerun Viewer to show incomplete or open ended video streams.
In contrast, [`AssetVideo`](../../reference/types/archetypes/asset_video.md) requires the entire
video asset file to be in Viewer memory before decoding can begin.

Refer to the [video camera streaming](https://github.com/rerun-io/rerun/blob/latest/examples/python/camera_video_stream) example to learn how to stream live video to Rerun.

Current limitations of `VideoStream`:
* [#9815](https://github.com/rerun-io/rerun/issues/9815): Decoding on native is generally slower than decoding in the browser right now.
  This can cause increased latency and in some cases may even stop video playback.
* [#10186](https://github.com/rerun-io/rerun/issues/10186): [`VideoStream`](../../reference/types/archetypes/video_stream.md) only supports H.264, H.265, AV1 at this point.
* [#10090](https://github.com/rerun-io/rerun/issues/10090): B-frames are not yet supported for [`VideoStream`](../../reference/types/archetypes/video_stream.md).
* [#10422](https://github.com/rerun-io/rerun/issues/10422): [`VideoFrameReference`](../../reference/types/archetypes/video_frame_reference.md) does not yet work with [`VideoStream`](../../reference/types/archetypes/video_stream.md).

<!--
Discoverable for scripts/zombie_todos.py:
TODO(#9815): fix above if ticket is outdated.
TODO(#10186): fix above if ticket is outdated.
TODO(#10090): fix above if ticket is outdated.
TODO(#10422): fix above if ticket is outdated.
-->

### Export MP4 from RRD (remuxing)

Sample data from [`VideoStream`](../../reference/types/archetypes/video_stream.md) can be queried
and remuxed to mp4 without re-encoding the video as demonstrated in [this sample](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/archetypes/video_stream_query_and_mux.py).

Check the [doc page on retrieving data](../../howto/query-and-transform/get-data-out.md) to learn more about dataframe queries in general.


## Video files

You can use [`AssetVideo`](../../reference/types/archetypes/asset_video.md) to log readily encoded video files.
Rerun ignores the timestamp at which the video asset itself is logged and requires you
to log [`VideoFrameReference`](../../reference/types/archetypes/video_frame_reference.md) to establish a
correlation of video time to the Rerun timeline.
To ease this, the SDK's `read_frame_timestamps_nanos` utility allows to read out timestamps from in-memory video assets:

snippet: archetypes/video_auto_frames

[#7354](https://github.com/rerun-io/rerun/issues/7354): Currently, only MP4 files are supported.

<!--
Discoverable for scripts/zombie_todos.py:
TODO(#7354): fix above if ticket is outdated.
-->

## Codec support in detail

### Overview

Codec support varies in the web & native viewer:

|            | Browser | Native |
| ---------- | ------- | ------ |
| AV1        | ✅       | 🟧      |
| H.264/avc  | ✅       | ✅      |
| H.265/hevc | 🟧       | ✅      |
| VP9        | ✅       | ❌      |

<!--
for web codecs see https://www.w3.org/TR/webcodecs-codec-registry/#video-codec-registry
VP8 is only not in the list because VP9 doesn't support MP4 as a container and that's
today the only container we take.
-->

Details see below.

When choosing a codec, we recommend [AV1](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs#av1),
as it seems to have the best overall playback support while also having very high compression quality.

Since AV1 can have very long encoding times, it is often not suitable for streaming.
In cases where encoding time matters, we recommend H.264/avc.

### Native viewer

#### AV1

AV1 is supported out of the box using a software decoder paired with gpu based image conversion.

Current limitations:
* [#7755](https://github.com/rerun-io/rerun/issues/7755): AV1 is supported on all native builds exception on Linux ARM.

<!--
Discoverable for scripts/zombie_todos.py:
TODO(#7755): fix above if ticket is outdated.
-->

#### H.264/avc & H.265/hevc

H.264/avc and H.265/hevc are supported via a separately installed `FFmpeg` binary, requiring a minimum version of `5.1`.

The viewer does intentionally not come bundled with `FFmpeg` to avoid licensing issues.
By default rerun will look for a system installed `FFmpeg` installation in `PATH`,
but you can specify a custom path in the viewer's settings.

If you select a video that failed to play due to missing or incompatible `FFmpeg` binaries it will offer a download link to a build of `FFmpeg` for your platform.
<!-- TODO(#8004): there should be a download button that updates the path in the settings -->

### Web viewer
Video playback in the Rerun Web Viewer is done using the browser's own video decoder, so the exact supported codecs depend on your browser.

Overall, we recommend using Chrome or another Chromium-based browser, as it seems to have the best video support as of writing.

For decoding video in the Web Viewer, we use the [WebCodecs API](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API).
This API enables us to take advantage of the browser's hardware accelerated video decoding capabilities.
It is implemented by all modern browsers, but with varying levels of support for different codecs, and varying levels of quality.

When it comes to codecs, we aim to support any codec which the browser supports, but
we currently cannot guarantee that all of them will work. For more information about
which codecs are supported by which browser, see [Video codecs on MDN](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs#codec_details).

We tested the following codecs in more detail:

|            | Linux Firefox | Linux Chrome[^1] | macOS Firefox | macOS Chrome | macOS Safari | Windows Firefox | Windows Chrome[^2] |
| ---------- | ------------- | ---------------- | ------------- | ------------ | ------------ | --------------- | ------------------ |
| AV1        | ✅             | ✅                | ✅             | ✅            | 🚧[^3]        | ✅               | ✅                  |
| H.264/avc  | ✅             | ✅                | ✅             | ✅            | ✅            | ✅               | ✅                  |
| H.265/hevc | ❌             | ❌                | ❌             | ✅            | 🚧[^4]        | ❌               | 🚧[^5]              |

[^1]: Any Chromium-based browser should work, but we don't test all of them.
[^2]: Chrome on Windows has been observed to stutter on playback. It can be mitigated by [using software decoding](https://rerun.io/docs/getting-started/troubleshooting#video-stuttering), but this may lead to high memory usage. See [#7595](https://github.com/rerun-io/rerun/issues/7595).
[^3]: Safari/WebKit does not support AV1 decoding except on [Apple Silicon devices with hardware support](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/).
[^4]: Safari/WebKit has been observed suttering when playing `hvc1` but working fine with `hevc1`. Despite support being advertised Safari 16.5 has been observed not support H.265 decoding.
[^5]: Only supported if hardware encoding is available. Therefore always affected by Windows stuttering issues, see above.

Beyond this, for best compatibility we recommend:
* prefer YUV over RGB & monochrome formats
* don't use more than 8bit per color channel
* keep resolutions at 8k & lower (see also [#3782](https://github.com/rerun-io/rerun/issues/3782))

## Other limitations
There are still some limitations to encoded Video in Rerun which will be addressed in the future:

* [#7594](https://github.com/rerun-io/rerun/issues/7594): HDR video is not supported
* [#5181](https://github.com/rerun-io/rerun/issues/5181): There is no audio support
* There is no video encoder in the Rerun SDK, so you need to create the video stream or file yourself.
  Refer to the [video camera streaming](https://github.com/rerun-io/rerun/blob/latest/examples/python/camera_video_stream) example to learn how to encode video using [`pyAV`](https://github.com/PyAV-Org/PyAV).

<!--
Discoverable for scripts/zombie_todos.py:
TODO(#7594): fix above if ticket is outdated.
TODO(#5181): fix above if ticket is outdated.
-->


## Links
* [Web video codec guide, by Mozilla](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs)
