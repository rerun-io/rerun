---
title: Generate segment URLs in dataframes
order: 80
---

The [`segment_url`](https://ref.rerun.io/docs/python/stable/common/utilities/#rerun.utilities.datafusion.functions.url_generation.segment_url) DataFusion utility can be used to generate Rerun URLs that are clickable within the viewer.
The generated URLs can optionally seek to a timestamp, select a time range, or select an entity path.

## Setup

We start by loading sample data in a local Data Platform instance and creating a table with some segment metadata.

snippet: howto/query-and-transform/segment_url[setup]

## Basic URL

With no extra arguments, `segment_url` produces a URL that opens the segment in the viewer.

snippet: howto/query-and-transform/segment_url[basic]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>
```

## Specify the time cursor position

Pass `timestamp` and `timeline_name` to generate a URL that tells the viewer to activate a specific timeline and set
the time cursor to a specific value.
If `timestamp` is a string, it will be interpreted as a column name.
Alternatively, any DataFusion expression can be provided, including a literal.

snippet: howto/query-and-transform/segment_url[timestamp]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>#when=real_time@2023-11-14T22:13:20Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>#when=real_time@2023-11-14T22:13:21Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>#when=real_time@2023-11-14T22:13:22Z
```

## Selecting a time range

Pass `time_range_start` and `time_range_end` together with `timeline_name` to generate a URL that specifies a time range to be selected.
Both can be a column name or a DataFusion expression.

snippet: howto/query-and-transform/segment_url[time_range]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>#time_selection=real_time@2023-11-14T22:13:20Z..2023-11-14T22:13:20.5Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>#time_selection=real_time@2023-11-14T22:13:21Z..2023-11-14T22:13:21.5Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>#time_selection=real_time@2023-11-14T22:13:22Z..2023-11-14T22:13:22.5Z
```

## Selecting an entity

Pass `selection` to generate a URL that specifies which entity path, instance, and/or component to select.
The value must be a string using entity path syntax, optionally followed by an instance index in brackets
and/or a component name after a colon.
For example: `/world/points`, `/world/points[#42]`, `/world/points:Color`, or `/world/points[#42]:Color`.

snippet: howto/query-and-transform/segment_url[selection]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>#selection=/camera/rgb
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>#selection=/observation/joint_positions
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>#selection=/observation/gripper_state
```

## Combining features

All three features can be used together. The generated URL includes every fragment that was specified.

snippet: howto/query-and-transform/segment_url[combined]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>#selection=/camera/rgb&when=real_time@2023-11-14T22:13:20Z&time_selection=real_time@2023-11-14T22:13:20Z..2023-11-14T22:13:20.5Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>#selection=/observation/joint_positions&when=real_time@2023-11-14T22:13:21Z&time_selection=real_time@2023-11-14T22:13:21Z..2023-11-14T22:13:21.5Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>#selection=/observation/gripper_state&when=real_time@2023-11-14T22:13:22Z&time_selection=real_time@2023-11-14T22:13:22Z..2023-11-14T22:13:22.5Z
```

## Using expressions

Every parameter that accepts a column name string also accepts an arbitrary DataFusion expression.
This is useful when you want to supply a constant value for all rows using `lit()` or build more advanced expressions.

snippet: howto/query-and-transform/segment_url[expressions]

Output:

```
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_1>#selection=/camera/rgb&when=real_time@2023-11-14T22:13:20Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_2>#selection=/camera/rgb&when=real_time@2023-11-14T22:13:21Z
rerun+http://localhost:51234/dataset/<DATASET_ID>?segment_id=<SEGMENT_ID_3>#selection=/camera/rgb&when=real_time@2023-11-14T22:13:22Z
```
