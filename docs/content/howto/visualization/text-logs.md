---
title: Show and filter text logs
order: 750
description: Log text messages, forward Python logging, and choose the columns and log levels shown in the text log view
---

## Log text messages

Log a [`TextLog`](../../reference/types/archetypes/text_log.md) with an optional level:

```python
rr.log("log/status", rr.TextLog("Application started.", level=rr.TextLogLevel.INFO))
```

The built-in levels are `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`, and `CRITICAL`.
You can also pass any other string as the level.

To forward Python `logging` to Rerun, add `rr.LoggingHandler`:

```python
import logging

logging.getLogger().addHandler(rr.LoggingHandler("logs"))
logging.getLogger().setLevel(logging.DEBUG)
```

## Configure the text log view

The [`TextLogView`](../../reference/types/views/text_log_view.md) shows the messages as a table.
In the blueprint, you can choose:

- `columns=rrb.TextLogColumns(…)`:
  - `timeline_columns`: which timelines to show as columns.
    By default, only the active timeline.
  - `text_log_columns`: which of the built-in columns to show, and in which order: `"loglevel"`, `"entitypath"`, and `"body"`.
- `rows=rrb.TextLogRows(filter_by_log_level=[…])`: which log levels to show.
  By default, all of them.
- `format_options=rrb.TextLogFormat(monospace_body=True)`: show the message in a monospace font.

snippet: views/text_log

You can change the same settings in the Viewer: select the view and use the selection panel.

## Limitations

- You cannot add your own columns (for example, a thread name or a source file).
  As a workaround, put that information into the message text, or log it to a separate entity path, which shows up in the `entitypath` column.
- There is no text search or filtering by message content.
  To show only some messages, split them into different entity paths, and use the view's [entity query](../../concepts/visualization/entity-queries.md) to choose which ones to show.

See [missing features](../../development/roadmap.md#missing-features).
