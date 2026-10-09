---
title: "Table filters can be defined in the table blueprint"
hidden: true
type: feature
---

### Table filters can be defined in the table blueprint

Table filters can now be defined in the table blueprint as SQL expressions.
Filters also still apply after you navigate back to a table, instead of only showing in the filter bar.

```python
rrb.TableBlueprint(filters=["\"task\" ILIKE '%pick%'", '"duration" > 10'])
```
