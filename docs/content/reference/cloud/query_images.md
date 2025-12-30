---
title: Query image & video types
order: 60
---

Images and videos are incredibly useful, however there are many ways to store and manipulate them.
For more details about the different video types we support see our [video reference](https://rerun.io/docs/reference/video).
This example focuses on querying image frames from the Rerun dataplatform.

The dependencies in this example require `rerun-sdk[all]`, and `av` for frame decoding.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your cloud instance.

snippet: reference/query_images[setup]

## Compressed Image

Compressed images are just stored as a string of bytes, so you can query them directly and transform back into a raw image.

snippet: reference/query_images[compressed_image]

## Raw Image

Raw images are stored in a flattened layout, so we need to reshape them.
These format details are written to the RRD when images are logged.

snippet: reference/query_images[raw_image]

## Video stream

Our videos are sent as a series of keyframes and delta frames.
To extract a specific frame, we must find the most recent keyframe and decode forward.
Keyframes currently aren't written automatically.

snippet: reference/query_images[video_stream]
