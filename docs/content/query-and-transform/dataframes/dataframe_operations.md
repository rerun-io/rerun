---
title: Common Dataframe Operations
order: 200
description: Run joins, filters, and aggregations on Rerun data with DataFusion
---

Rerun provides a dataframe interface to your data via [DataFusion](https://datafusion.apache.org/python/).
This example runs joins, filters, and aggregations on a recording.
DataFusion executes lazily, so processing data in DataFusion is usually faster than converting it first.
You can still convert to dataframes for other tools (pandas, polars, pyarrow).

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

The dependencies in this example are contained in `rerun-sdk[all]`.

## Setup

Perform initial import and spawn local server for demonstration.
In practice you'll connect to your catalog server, such as Rerun Hub.

snippet: howto/dataframe_operations[setup]

## Group-by / aggregation

Perform an aggregation on the episodes to track the first and last timestamp for the columns of interest.

snippet: howto/dataframe_operations[group_by]

## Join and query

Some of our columns start much later than others.
Find out how often this delay exceeds some threshold.

> [!NOTE]
> Even though datafusion pulls data lazily, we don't currently decouple our payload from its timeline.
> E.g. in this example this means that we have to pull the full camera data to inspect their min/max timestamps.
> This works quickly when the data is already local and in memory, but can be a bottleneck on cloud at scale.

snippet: howto/dataframe_operations[join_query]

## Extract sub-episodes from recording

Oftentimes a recording will capture multiple episodes.
For instance a robotic arm may place multiple items, where each item could be considered an episode.
This example looks for contiguous time ranges where the gripper opens and closes in order to separate these sub-episodes for further downstream processing.

snippet: howto/dataframe_operations[sub_episodes]
