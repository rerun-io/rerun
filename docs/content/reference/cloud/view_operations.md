---
title: View Operations
order: 70
---

Robotics data has many sensors and many columns.
In order to more narrowly specify relevant content for further dataframe operations you first generate a view.
This view can filter on episode, time, column name etc.
This example shows specific instances highlighting these capabilities.

The dependencies in this example are contained in `rerun-sdk[all]`.

## Setup

Simplified setup.
Perform imports and launch local server for demonstration.
Extract an initial view `observations` that we later refine before generating a dataframe.

snippet: reference/view_operations[setup]

## Filtering on episode and time

Limit the scope of the view so that only a subset of data will ever be considered client side.
Pick a specific episode by id, and a time range.

snippet: reference/view_operations[filtering]

## Querying static data

So far we've been selecting `real_time` as the index. However, some data might not change in time (e.g. static transformations)
and thus isn't aligned with a time index. We specify this static timeline with `None`.

snippet: reference/view_operations[static_data]
