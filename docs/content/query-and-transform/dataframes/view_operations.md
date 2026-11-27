---
title: View Operations
order: 300
description: Filter by episode, time, and column to focus a dataframe
---

Robotics data has many sensors and many columns.
To narrow down the content for further dataframe operations, you first create a view.
A view can filter on episode, time, and column name.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

The dependencies in this example are contained in `rerun-sdk[all]`.

## Setup

Simplified setup.
Perform imports and launch local server for demonstration.
Extract an initial view `observations` that we later refine before generating a dataframe.

snippet: howto/view_operations[setup]

## Filtering on episode and time

Limit the scope of the view so that only a subset of data will ever be considered client side.
Pick a specific episode by id, and a time range.

snippet: howto/view_operations[filtering]

## Querying static data

So far we've been selecting `real_time` as the index. However, some data might not change in time (e.g. static transformations)
and thus isn't aligned with a time index. We specify this static timeline with `None`.

snippet: howto/view_operations[static_data]
