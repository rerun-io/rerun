---
title: Time-align data
order: 80
---

Real-world data is usually not time-aligned.
Rerun provides capabilities to simplify time alignment.
One common use case is to fill forward to run compute at a fixed frequency.
This example demonstrates how Rerun simplifies that process.

The dependencies in this example require `rerun-sdk[all]`, and `pandas` because python datetimes only support microsecond precision.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your cloud instance.

snippet: howto/time_alignment[setup]

## Extract desired timepoints

Select start and end time of data, downsample to a fixed frequency, and specify those as the desired output timestamps.

snippet: howto/time_alignment[extract_timepoints]

## Time-align data

Select the timeline, columns, and episode of interest.
Extract rows at the specified time points, and fill forward to eliminate sparse entries.
Finally, filter out nulls for initial sensor state that cannot be resolved with forward fill.

snippet: howto/time_alignment[time_align]
