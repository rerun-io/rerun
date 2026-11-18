---
title: Curating annotated time spans
order: 125
description: Annotate and query time spans within recordings
---

Annotate time spans within recordings with labels, scores, or other data.
Spans can:

- Leave gaps between annotations.
- Overlap other spans.
- Nest within other spans to represent hierarchical tasks and subtasks.

Use the experimental `SpanTable` API to create, edit, and query these annotations.

## Create an annotation table

The examples assume a connected `catalog` client and an existing dataset named `robot_recordings`, containing recording `episode_1` with a duration timeline named `time`.
Each span includes at least these columns:

| Column | Arrow type | Contents |
|---|---|---|
| `rerun_segment_id` | `string` | ID of the recording containing the span |
| `rerun_span_id` | `string` | Unique ID of the span, generated as a UUID when omitted |
| `<timeline>:start` | `duration[ns]` or `timestamp[ns]` | Required inclusive start |
| `<timeline>:end` | Same type as start | Required inclusive end |

Each table uses one duration or timestamp timeline, chosen with `index`.
Its boundaries use the timeline's nanosecond-resolution Arrow type.
Use `schema` to declare additional data columns, such as `state` and `score`:

snippet: howto/spans[create]

Omit `schema` if you do not need additional data columns.
Sequence timelines, including frame indices, are not supported.
Data columns and the timeline are fixed at creation.
By default, the table appears in the catalog's `spans` folder.
Retrieve it later with `SpanTable.open(dataset)`.
To keep several span tables for one dataset, pass the same `name` to `SpanTable.create` and `SpanTable.open`.

## Add or edit spans

Pass inclusive `start` and `end` boundaries, with `start <= end`.
On duration timelines, use `datetime.timedelta` or `numpy.timedelta64` values.
On timestamp timelines, use `datetime.datetime` or `numpy.datetime64` values; naive datetimes are treated as UTC.
Pass column values through `data`:

snippet: howto/spans[add_or_edit]

Here, `state` and `score` are separate columns matching the declared schema; undeclared data columns are rejected.
Keep the generated span ID to edit the same row later.
Upsert requires a span ID.
Upsert replaces the whole row, so omitted nullable fields become null; an unknown ID inserts a new span.

Use `annotations.write(rows, mode="append")` for bulk Arrow writes; `"upsert"` and `"overwrite"` are also supported.
`"overwrite"` replaces every span in the table, not only the rows with matching IDs.
Supply `rerun_segment_id` and typed `<timeline>:start` / `<timeline>:end` columns, such as `time:start` and `time:end`.
Both boundary columns are required on every row and use `duration[ns]` or `timestamp[ns]` to match the table schema.
Missing span IDs are generated automatically.
Bulk writes don't check that caller-supplied span IDs are unique; appending an existing ID creates a duplicate row.
Every write rejects segment IDs that are not in the dataset.

## Query the annotation table

`reader()` returns a DataFusion DataFrame containing span columns and recording properties.
Filter it with DataFusion expressions, then collect results using the DataFrame's `to_arrow_table()` method:

snippet: howto/spans[query]

Rows with missing `state` or `score` values do not pass the filter above.
Filters keep or discard whole spans without changing boundaries or subtracting overlaps.
Selecting spans does not require loading the recordings.

Filter the segment ID column to select spans from one recording:

snippet: howto/spans[filter_segment]

Add a `pick_and_place` task enclosing the `grasp` span above, then find grasps fully contained in a task from the same recording:

snippet: howto/spans[hierarchy]

Each matching grasp is returned once, even if multiple tasks contain it.

## Delete the annotation table

Delete the annotations without deleting the dataset or its recordings:

snippet: howto/spans[delete]

Deleting the dataset does not delete its annotation table.
