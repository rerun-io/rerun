---
title: Query images
order: 100
description: Query image columns from a catalog server and decode the image frames
---

This example shows how to query image frames from a catalog server.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

The dependencies in this example require `rerun-sdk[all]`.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your catalog server, such as Rerun Hub.

snippet: howto/query_images[setup]

## Compressed image

Compressed images are just stored as a string of bytes, so you can query them directly and transform back into a raw image.

snippet: howto/query_images[compressed_image]

## Raw image

Raw images are stored in a flattened layout, so we need to reshape them.
These format details are written to the RRD when images are logged.

snippet: howto/query_images[raw_image]
