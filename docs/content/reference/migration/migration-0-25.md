---
title: Migrating from 0.23 to 0.24
order: 985
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Changed arrow encoding of blobs
We used to encode blobs as `List<uint8>`, which was rather unidiomatic.
Now they are instead encoded as `Binary`.
Old data will be migrated on ingestion (zero-copy).

Affects the following components:
- [`Blob`](https://rerun.io/docs/reference/types/components/blob)
- [`ImageBuffer`](https://rerun.io/docs/reference/types/components/image_buffer)
- [`VideoSample`](https://rerun.io/docs/reference/types/components/video_sample)
