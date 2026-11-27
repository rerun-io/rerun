---
title: Browse and annotate tables
order: 200
description: Explore arbitrary tabular data and link rows to recordings
---

Tables let you bring arbitrary tabular data into the Viewer, such as evaluation metrics, review status, and notes alongside links to recordings.
You can browse rows, filter and sort columns, and open linked recordings for a closer look.

## Bring a table into the Viewer

Use [`send_table`](../../log/send-data/send-table.md) to send a table from code directly to a running Viewer.
It appears in Sources alongside recordings and is useful for exploring data in that session.

To store query results or review data for later use, create or register a [catalog table](../../catalog/catalog-tables.md).
Connect the Viewer to its catalog server and select the table in Sources.

## Link rows to recordings

A table can include a recording URI for each row, for example a segment URL obtained with `dataset.segment_url(segment_id)`.
The Viewer can display the URI as a link that opens the recording.
A card layout can use the same URI as its link, and a table blueprint can turn the column into an embedded recording preview.
The dataset's own segment table provides a `recording link` column for this purpose.
See [Configure table layouts and segment previews](table-layout.md#defining-a-preview-column) for a complete example.

## Flag segments and add notes

To mark segments as keep or discard, correct or incorrect, or add notes, use editable columns in a catalog table with a row key and write access.
Boolean columns can be shown as flags, and editable text columns can hold notes.
A row can link to a segment, so you can inspect the recording while reviewing it.

<video width="100%" autoplay loop muted controls>
  <source src="https://static.rerun.io/5f7a1167b2174451397129941509c166b25ef807_table_column_types_v2.mp4" type="video/mp4" />
</video>

Configure columns as editable in the table schema or a table blueprint, then edit the cells in the Viewer.
Create the table and add its rows from the SDK; the Viewer does not add rows or columns.
Editing is not supported on a dataset's segment table.
See [Editing columns](table-layout.md#editing-columns) for the setup and example.
