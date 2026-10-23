---
title: Chained `filter_contents()` calls intersect
hidden: true
type: breaking
---

### Chained `filter_contents()` calls intersect

Calling `filter_contents()` on a `DatasetSelection` now narrows it, as chained `filter_segments()` calls already do: an entity is kept only if every call selects it.
Previously, each call added its expressions to the earlier ones, so a later call could widen the selection or include an entity that an earlier call excluded.
Expressions passed in a single call behave as before.

Since each call is evaluated on its own, a call with only exclusions selects nothing.
To combine several subtrees, or includes with excludes, pass them all in one call.
To exclude entities from an existing selection, start from everything and exclude from there:

```python
# Before: selected both subtrees. Now: selects nothing.
selection = dataset.filter_contents("/robot/**").filter_contents("/camera/**")

# Selects both subtrees, before and now.
selection = dataset.filter_contents(["/robot/**", "/camera/**"])

# Before: selected `/robot/**` without `/robot/debug/**`. Now: selects nothing.
selection = dataset.filter_contents("/robot/**").filter_contents("-/robot/debug/**")

# Selects `/robot/**` without `/robot/debug/**`, before and now.
selection = dataset.filter_contents(["/robot/**", "-/robot/debug/**"])

# Selects `/robot/**` without `/robot/debug/**`, narrowing an existing selection.
robot = dataset.filter_contents("/robot/**")
selection = robot.filter_contents(ContentFilter.everything().exclude("/robot/debug/**"))
```
