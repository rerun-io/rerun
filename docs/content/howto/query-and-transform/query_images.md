---
title: Query images
order: 60
---

Images are incredibly useful, however there are many ways to store and manipulate them.
This example focuses on querying image frames from the Rerun Data Platform.

The dependencies in this example require `rerun-sdk[all]`.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your cloud instance.

snippet: howto/query_images[setup]

## Compressed image

Compressed images are just stored as a string of bytes, so you can query them directly and transform back into a raw image.

snippet: howto/query_images[compressed_image]

## Raw image

Raw images are stored in a flattened layout, so we need to reshape them.
These format details are written to the RRD when images are logged.

snippet: howto/query_images[raw_image]
