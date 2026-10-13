---
title: "Table filters can be defined in the table blueprint and run faster"
hidden: true
type: feature
---

### Table filters can be defined in the table blueprint and run faster

Table filters can now be defined in the table blueprint as SQL expressions.
Filters also still apply after you navigate back to a table, instead of only showing in the filter bar.

```python
rrb.TableBlueprint(filters=["\"task\" ILIKE '%pick%'", '"duration" > 10'])
```

As a side-effect of using SQL for filters is that they're also much faster in many cases. About 1000x faster for timestamp columns, and 20-300x faster on list columns.
