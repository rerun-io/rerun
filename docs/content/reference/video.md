---
title: Video
---

## Video in Rerun
A stream of images (like those produced by a camera) can be logged to Rerun in several different ways:

* Uncompressed, as many [`Image`](../reference/types/archetypes/image.md)s
* Compressed as many [`EncodedImage`](../reference/types/archetypes/encoded_image.md)s, using e.g. JPEG-encoding
* Compressed as a single [`AssetVideo`](../reference/types/archetypes/asset_video.md), using e.g. MP4 and AV1 or H.264.

These alternatives range on a scale of "simple, lossless, and big" to "complex, lossy, and small".

If you want lossless encoded images (with no compression artifacts), then you should log each video frame as `Image`.
This will use up a lot of space and bandwidth. You can also encode them as PNG and log them as `EncodedImage`,
though it should be noted that PNG encoding usually does very little for the file size of photographic images.

If you want to reduce bandwidth and storage cost, you can encode each frame as a JPEG and log it using `EncodedImage`. This can easily reduce the file sizes by almost two orders of magnitude with minimal perceptiall loss.
This is also very simple to do, and the Python logging SDK has built-in support for it using [`Image.compress`](https://ref.rerun.io/docs/python/0.18.2/common/archetypes/#rerun.archetypes.Image.compress).

Finally, you can encode the images as a video file, and log it using `AssetVideo`.
This gives the best compression ratio, reducing file sizes and bandwidth requirements.

## `AssetVideo` limitations
Video support is new in Rerun, and has several limitations:

* Video playback only works in the web viewer
* There is no video encoder in the Rerun SDK, so you need to create the video file yourself
* Only the MP4 container format is supported
* Only a limited sets of codecs are supported (see below)
* There is no audio support

## Web viewer support
As of writing, playback of `AssetVideo` is only supported on the web viewer.
Native video playback is coming, and can be tracked [in this GitHub issue](https://github.com/rerun-io/rerun/issues/7298).

Video playback is done using the browsers own video decoder, so the supported codecs depends on your browser.

Overall, we recommend using Chrome or another Chromium-based browser, as it seem to have the best video support as of writing.

When choosing codec, we recommend [AV1](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs#av1), as that seems to have the best overall playback support. Since AV1 is patent-free, it is also likely the first codec we will support in the native viewer.

## Links
* [Web video codec guide, by Mozilla](https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Video_codecs)
