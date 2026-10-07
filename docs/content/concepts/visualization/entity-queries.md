---
title: Entity Queries
order: 400
description: Control which entities appear in a given view
---

Many views are made up of visualizations that include more than one
entity.

Rather that requiring you to specify each entity individually, Rerun supports
this through "entity queries" that allow you to use "query expressions" to
include or exclude entire subtrees.

## Query expression syntax

Query expressions use the [entity path filter](../logging-and-ingestion/entity-path.md#entity-path-filters) syntax.

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
(
    rrb.Spatial3DView(
        contents=[
            "+ helix/**",
            "- helix/structure/scaffolding",
        ],
    ),
)
```

## `origin` substitution

Query expressions also allow you to use the variable `$origin` to refer to the
origin of the view that the query belongs to.

For example, the above query could be rewritten as:

```python
(
    rrb.Spatial3DView(
        origin="helix",
        contents=[
            "+ $origin/**",
            "- $origin/structure/scaffolding",
        ],
    ),
)
```
