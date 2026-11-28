---
title: Time-align data
order: 400
description: Fill forward unaligned data so you can run compute at a fixed frequency
---

Real-world data is usually not time-aligned.
This example fills data forward so that you can run compute at a fixed frequency.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

The dependencies in this example require `rerun-sdk[all]`, and `pandas` because python datetimes only support microsecond precision.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your catalog server, such as Rerun Hub.

snippet: howto/time_alignment[setup]

## Extract desired timepoints

Select start and end time of data, downsample to a fixed frequency, and specify those as the desired output timestamps.

snippet: howto/time_alignment[extract_timepoints]

## Time-align data

Select the timeline, columns, and episode of interest.
Extract rows at the specified time points, and fill forward to eliminate sparse entries.
Finally, filter out nulls for initial sensor state that cannot be resolved with forward fill.

> [!NOTE]
> Video samples can only be decoded starting from the previous keyframe, so a forward-filled, resampled `VideoStream:sample` column cannot be decoded.
> For video, use the keyframe-aware approach in [Query video streams](../images-and-video/query_videos.md) or the [dataloader](../../train/dataloader.md).

snippet: howto/time_alignment[time_align]
