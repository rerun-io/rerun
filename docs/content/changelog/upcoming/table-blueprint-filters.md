---
title: "Table filters can be defined in the table blueprint and run faster"
hidden: true
type: feature
---

### Table filters can be defined in the table blueprint and run faster

Table filters can now be defined in the table blueprint. Either using `rrb.table.col` to get filters that the viewer UI can edit, or by passing an SQL expression.

```python
import rerun.blueprint as rrb
from rerun.blueprint.table import col

rrb.TableBlueprint(
    # The blueprint doesn't know the schema of your table, so you have
    # to make sure the filters make sense for each column.
    filters=[
        col("task").str.contains("pick"),
        col("success").is_true(),
        col("created").last_week(),
        col("tags").list.str.does_not_contain("broken"),
        'length("notes") > 0',  # Can also pass SQL expressions.
    ],
)
```

As a side-effect of using SQL for filters is that they're also much faster in many cases. About 1000x faster for timestamp columns, and 20-300x faster on list columns.

See [Filtering rows](../howto/visualization/configure-table-blueprints.md#filtering-rows) for the full list of filters.
