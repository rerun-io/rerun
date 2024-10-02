---
title: Video in Rerun
order: 400
---

A stream of images (like those produced by a camera) can be logged to Rerun in several different ways:

* Uncompressed, as many [`Image`](../reference/types/archetypes/image.md)s
* Compressed as many [`EncodedImage`](../reference/types/archetypes/encoded_image.md)s, using e.g. JPEG.
* Compressed as a single [`AssetVideo`](../reference/types/archetypes/asset_video.md), using e.g. MP4.

These alternatives range on a scale of "simple, lossless, and big" to "complex, lossy, and small".

If you want lossless encoded images (with no compression artifacts), then you should log each video frame as `Image`.
This will use up a lot of space and bandwidth. You can also encode them as PNG and log them as `EncodedImage`,
though it should be noted that PNG encoding usually does very little for the file size of photographic images.

If you want to reduce bandwidth and storage cost, you can encode each frame as a JPEG and log it using `EncodedImage`. This can easily reduce the file sizes by almost two orders of magnitude with minimal perceptual loss.
This is also very simple to do, and the Python logging SDK has built-in support for it using [`Image.compress`](https://ref.rerun.io/docs/python/0.18.2/common/archetypes/#rerun.archetypes.Image.compress).

Finally, you can encode the images as a video file, and log it using `AssetVideo`.
This gives the best compression ratio, reducing file sizes and bandwidth requirements.

snippet: archetypes/video_auto_frames

## `AssetVideo` limitations
Video support is new in Rerun, and has several limitations:

* [#7298](https://github.com/rerun-io/rerun/issues/7298): Video playback only works in the web viewer
* [#7354](https://github.com/rerun-io/rerun/issues/7354): Only the MP4 container format is supported
* [#5181](https://github.com/rerun-io/rerun/issues/5181): There is no audio support
* There is no video encoder in the Rerun SDK, so you need to create the video file yourself
* Only a limited sets of codecs are supported (see below)

## Web viewer support
As of writing, playback of `AssetVideo` is only supported on the web viewer.
Native video playback is coming, and can be tracked [in this GitHub issue](https://github.com/rerun-io/rerun/issues/7298).

Video playback is done using the browser's own video decoder, so the supported codecs depend on your browser.

Overall, we recommend using Chrome or another Chromium-based browser, as it seems to have the best video support as of writing.

When choosing a codec, we recommend [AV1](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs#av1), as that seems to have the best overall playback support. Since AV1 is patent-free, it is also likely the first codec we will support in the native viewer.

For decoding video in the Web Viewer, we use the [WebCodecs API](https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API).
This API enables us to take advantage of the browser's hardware accelerated video decoding capabilities.
It is implemented by all modern browsers, but with varying levels of support for different codecs, and varying levels of quality.

With that in mind, here are the browsers which we have tested and verified to generally work:

|            | Linux  | macOS | Windows |
| ---------- | ------ | ----- | ------- |
| Firefox    | ✅[^1] | ✅    | ✅      |
| Chrome[^2] | ✅     | ✅    | ✅[^3]  |
| Safari     |        | ✅    |         |

[^1]: Firefox on Linux has been observed to [stutter when playing back H.264 video](https://github.com/rerun-io/rerun/issues/7532).
[^2]: Any Chromium-based browser should work, but we don't test all of them.
[^3]: Chrome on Windows has also been observed to stutter on playback. It can be mitigated by [using software decoding](https://rerun.io/docs/getting-started/troubleshooting#video-stuttering).

When it comes to codecs, we aim to support any codec which the browser supports, but
we currently cannot guarantee that all of them will work. For more information about
which codecs are supported by which browser, see [Video codecs on MDN](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs#codec_details).

At the moment, we test the following codecs:

|           | Linux Firefox | Linux Chrome | macOS Firefox | macOS Chrome | macOS Safari | Windows Firefox | Windows Chrome |
| --------- | ------------- | ------------ | ------------- | ------------ | ------------ | --------------- | -------------- |
| AV1       | ✅            | ✅           | ✅           | ✅           | 🚧[^4]       | ✅             | ✅            |
| H.264     | ✅            | ✅           | ✅           | ✅           | ✅           | ✅             | ✅            |
| H.265     | ❌            | ❓           | ❌           | ✅           | 🚧[^6]       | ❌             | ❓            |
 
[^4]: Safari/WebKit does not support AV1 decoding except on [Apple Silicon devices with hardware support](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/).
[^5]: Firefox does not support H.265 decoding on any platform.
[^6]: Safari/WebKit has been observed suttering when playing `hvc1` but working fine with `hevc1`.


## Links
* [Web video codec guide, by Mozilla](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs)
