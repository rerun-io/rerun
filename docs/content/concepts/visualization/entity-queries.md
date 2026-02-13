---
title: Entity Queries
order: 400
---

Many views are made up of visualizations that include more than one
entity.

Rather that requiring you to specify each entity individually, Rerun supports
this through "entity queries" that allow you to use "query expressions" to
include or exclude entire subtrees.

## Query expression syntax

An entity query is made up of a set of "query expressions." Each query expression
is either an "inclusion," which starts with an optional `+` or an "exclusion,"
which always starts with a `-`.

Query expressions are also allowed to end with an optional `/**`. The`/**`
suffix matches the whole subtree, i.e. self and any child, recursively. For
example, `/world/**`matches both`/world`and`/world/car/driver`. Other uses of
`*` are not yet supported.

When combining multiple query expressions, the rules are sorted by entity-path,
from least to most specific:

-   If there are multiple matching rules, the most specific rule wins.
-   If there are multiple rules of the same specificity, the last one wins.
-   If no rules match, the path is excluded.

Consider the following example:

```diff
+ /world/**
- /world
- /world/car/**
+ /world/car/driver
```

-   The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it
    is included.
-   The last rule matching `/world/car/hood` is `- /world/car/**`, so it is
    excluded.
-   The last rule matching `/world` is `- /world`, so it is excluded.
-   The last rule matching `/world/house` is `+ /world/**`, so it is included.

## In the Viewer

In the viewer, an entity query is typically displayed as a multi-line
edit box, with each query expression shown on its own line. You can find the
query editor in the right-hand selection panel when selecting a view.

<picture>
  <img src="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/1200w.png">
</picture>

## In the SDK

In the SDK, query expressions are represented as a list or iterable, with each
expression written as a separate string. The query expression from above would
be written as:

```python
rrb.Spatial3DView(
    contents=[
        "+ helix/**",
        "- helix/structure/scaffolding",
    ],
),
```

## `origin` substitution

Query expressions also allow you to use the variable `$origin` to refer to the
origin of the view that the query belongs to.

For example, the above query could be rewritten as:

```python
rrb.Spatial3DView(
    origin="helix",
    contents=[
        "+ $origin/**",
        "- $origin/structure/scaffolding",
    ],
),
```
