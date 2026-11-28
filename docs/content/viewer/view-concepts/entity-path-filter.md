---
title: Entity path filter
order: 300
description: Choose which entities a view displays with include and exclude rules
---

Use an entity path filter to focus a view on part of your recording, such as one camera or a robot's trajectory, while leaving out unrelated or debug data.
The filter selects entities by their paths without changing the recording, so different views can show different subsets of the same data.

A filter is a list of include and exclude rules.
Each rule can match one entity or an entire subtree, so you do not need to list every entity separately.

## View origin

A view's `origin` is an entity path that filter rules can refer to as [`$origin`](#origin-substitution).

In 2D and 3D views, the origin also gives the default [target frame](../../reference/types/views/spatial3d_view.md#spatial_information): the frame that the view shows all data in.
Data gets to that frame through [transforms](../../log/data-model/transforms.md), which come from the entity hierarchy or from named transform frames.

You can now set the target frame directly, so the transforms in a view no longer depend on its origin.
For this reason, the `origin` will be deprecated in an upcoming version.

## Filter syntax

Use `+` to include a path, `-` to exclude it, and `/**` to match a path and all its descendants.
See [entity path filter syntax](../../log/data-model/entity-path.md#entity-path-filters) for how matching rules are combined.

## In the Viewer

Select a view to edit its entity path filter in the right-hand selection panel.
The filter editor shows one rule per line.

<picture>
  <img src="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/full.png" alt="Rerun Viewer with a 3D helix view selected and the entity path filter in the selection panel including helix/** and excluding scaffolding">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/helix_query/e39ed9fa364724d201f19a0ae54f34d4df761c5b/1200w.png">
</picture>

## In the SDK

Set the view's `contents` parameter to a list or iterable of filter rules, with each rule written as a separate string.
The filter shown above would be written as:

```python
rrb.Spatial3DView(contents=["+ helix/**", "- helix/structure/scaffolding"])
```

## `origin` substitution

Filter rules can use `$origin` to refer to the view's `origin`.

For example, the filter above could be rewritten as:

```python
rrb.Spatial3DView(origin="helix", contents=["+ $origin/**", "- $origin/structure/scaffolding"])
```

## Recording properties are excluded by default

Every view's entity path filter implicitly excludes `__properties/**`, where [recording properties](../../catalog/properties-and-segments.md) are stored.
So `+ /**` does not show them.
To include them in a view, add the rule `+ __properties/**` to the view's contents explicitly.

## Hide or exclude an entity

Use the eye icon in the blueprint panel to hide or show an entity interactively.
To hide an entity (and its children) by default in a view, use the [`EntityBehavior`](https://ref.rerun.io/docs/python/stable/common/blueprint_archetypes/#rerun.blueprint.archetypes.EntityBehavior) blueprint archetype as an override.
The entity is still part of the view, so the user can show it again by clicking the eye icon in the blueprint panel.
You can also make entities non-interactive, so that they cannot be hovered or selected in the view.

snippet: archetypes/entity_behavior

`EntityBehavior` currently only has an effect when you set it in the blueprint, as above.
Logging it together with your data does nothing.

To remove an entity from a view completely, exclude it in the view's [entity path filter](entity-path-filter.md) instead, for example with `contents=["+ /**", "- /debug/**"]`.
