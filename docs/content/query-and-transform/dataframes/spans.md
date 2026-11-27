---
title: Curating and training from annotated time spans
order: 125
description: Annotate, query, curate and train from time spans within recordings.
---

Span tables attach properties to time spans of a dataset's segments.
This page shows how to create, edit, and query span annotations, and how to use them to curate training data.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../../catalog/hub.md) ([how to switch](../../catalog/hub.md#run-the-examples-on-hub)).

We consider that time spans are a different higher level concept with these properties:

- Time spans are not necessarily contiguous, there can be gaps between two consecutive time spans.
- Multiple time spans can overlap.
- Spans can nest within other spans to represent hierarchical tasks and subtasks.
- We can attach any property to a time span, a text, score, embedding, or any other metadata.

Time span properties are here implemented through an experimental `SpanTable` API to create, edit, and query these time span properties. The data is not duplicated, we are simply referencing a time interval for a given episode.

The data flywheel is hence implemented by:
- Annotate the data by defining time spans with properties
- Query the span table to understand the dataset
- Query the span table to isolate a subset of the data defined by a list of time spans
- Train directly using the rerun dataloader from that curated subset of data

## Create an annotation table

A span table is always related to a particular dataset, and has a semi fixed schema. The `rerun_segment_id`, `rerun_span_id`, `<timeline>:start`, `<timeline>:end` are mandatory columns for the span table.

| Column | Arrow type | Contents |
|---|---|---|
| `rerun_segment_id` | `string` | ID of the segment containing the span |
| `rerun_span_id` | `string` | Unique ID of the span |
| `<timeline>:start` | `int64`, `duration[ns]`, or `timestamp[ns]` | Required inclusive start |
| `<timeline>:end` | Same type as start | Required inclusive end |

While these columns are fixed, it is possible to add more columns to the table schema when creating the span table through the API. The API also forces the choice of timeline for the table, using the `index` parameter.
Its boundaries use the timeline's Arrow type: `int64` steps on sequence timelines and nanoseconds on duration and timestamp timelines.

snippet: howto/spans[create]

Omit `schema` if you do not need additional data columns.
Data columns and the timeline are fixed at creation.
By default, the table appears in the catalog's `spans` folder.
Retrieve it later with `SpanTable.open(dataset)`.
To keep several span tables for one dataset, pass the same `name` to `SpanTable.create` and `SpanTable.open`.

## Add or edit spans

The first condition to edit or add new spans to a span table is to have a handle on the span table with `SpanTable.open(dataset)`. Then we can use the `write` API.

snippet: howto/spans[add_or_edit]

To specify the time boundaries of a span, pass inclusive `start` and `end` boundaries, with `start <= end`.
On sequence timelines, use `int` or `numpy.integer` values.
On duration timelines, use `datetime.timedelta` or `numpy.timedelta64` values.
On timestamp timelines, use `datetime.datetime` or `numpy.datetime64` values; naive datetimes are treated as UTC.
`append` generates and returns a span ID; `upsert` requires one.

Pass column values through `data`.
In the example above, `state` and `score` are separate columns matching the declared schema; undeclared data columns are rejected.
Upsert replaces the whole row, so omitted nullable fields become null; an unknown ID inserts a new span.

Use `annotations.write(rows, mode="append")` for bulk Arrow writes; `"upsert"` and `"overwrite"` are also supported.
`"overwrite"` replaces every span in the table, not only the rows with matching IDs.
Supply `rerun_segment_id` and typed `<timeline>:start` / `<timeline>:end` columns, such as `time:start` and `time:end`.
Both boundary columns are required on every row and use `int64`, `duration[ns]`, or `timestamp[ns]` to match the table schema.
Integer boundary columns are also accepted on every timeline, as nanoseconds on duration and timestamp timelines, and timestamp timelines also accept dates.
Any other boundary type, such as timestamps on a sequence timeline, is rejected.
Bulk `append` and `overwrite` generate span IDs; caller-supplied IDs are only accepted by `upsert`.
Bulk `upsert` requires nonempty, non-null IDs; uniqueness within the batch is the caller's responsibility.
Every write rejects segment IDs that are not in the dataset.

## Query the annotation table

`reader()` returns a DataFusion DataFrame containing span columns and recording properties.
Filter it with DataFusion expressions, then collect results using the DataFrame's `to_arrow_table()` method:

snippet: howto/spans[query]

Rows with missing `state` or `score` values do not pass the filter above.
Filters keep or discard whole spans without changing boundaries or subtracting overlaps.
Selecting spans does not require loading the segments' data.

### Examples

Filter the segment ID column to select spans from one segment:

snippet: howto/spans[filter_segment]

Add a `pick_and_place` task enclosing the `grasp` span above, then find grasps fully contained in a task from the same segment:

snippet: howto/spans[hierarchy]

Each matching grasp is returned once, even if multiple tasks contain it.

## Select spans for training

Span-level curation filters the annotation table to choose intervals within segments.
Dataset-level queries choose segments; queries on the annotation table choose intervals within them.
Combine both selections before saving the training manifest:

snippet: howto/spans[curation]

`to_curation_arrow` exports segment IDs and typed boundaries, together with dataset and timeline identity.
`Curation.from_source` resolves their intersection with the view's segment ranges before sampling, without fetching segment data.
`Curation` removes duplicate intervals and leaves overlapping intervals distinct.
Without `spans`, `Curation.from_source(view, index="time")` curates the selected segments' full ranges without requiring a span table.
Pass it through `DataSource(dataset, curation=curation)` to either dataloader; see [Train on selected spans](../../train/dataloader.md#train-on-selected-spans).
Intervals select sample anchors; field windows may extend beyond them. That means depending on the observation history, some observations for given anchors may fall outside of the specified time spans.
The resolved Arrow rows are the training manifest; the original recording-selection view is not needed to reload it.
Content filters remain loader configuration, alongside fields, and are not stored in a curation.

## Save a curation

Save the selected coordinates under a dataset-scoped tag to reuse them later:

```python
Curation.save(dataset, curation.to_arrow(), tag="training-v1")
```

Reload that selection without repeating the query or local curation:

```python
curation = Curation.open(dataset, tag="training-v1")
coordinates = curation.to_arrow()
```

Tags are case-sensitive; saving an existing tag raises `AlreadyExistsError`.
Saved coordinates remain available after editing or deleting the annotation table.
Saved curations are ordinary catalog tables; these helpers never overwrite them, but direct table APIs can still modify or delete them.
For local persistence, use `curation.write_parquet("training.parquet")` and `Curation.from_parquet(dataset, "training.parquet")`.

## Delete the annotation table

Delete the annotations without deleting the dataset or its segments:

snippet: howto/spans[delete]

Deleting the dataset does not delete its annotation table.
