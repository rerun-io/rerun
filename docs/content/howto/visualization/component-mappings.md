---
title: Component mappings
order: 250
---

By default, each visualizer reads its input components from the - for example, the `Points3D` visualizer reads colors from `Points3D:colors`.
**Component mappings** let you override this, redirecting any visualizer input to a different component on the same entity. This makes it possible to store multiple variants of the same data and switch between them per view.

To learn more about how visualizers are set up in general, also have a look at the [concept page on customizing Views](https://landing-4rzjg7apg-rerun.vercel.app/docs/concepts/visualization/customize-views#speculative-link).

This guide uses a point cloud with two color sets as a running example, but the same technique works for any component!

## Full example

You can find the full example here:

* 🐍 [Python](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/dual_color_point_cloud.py)
* 🦀 [Rust](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/dual_color_point_cloud.rs)

<picture>
  <img src="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/1200w.png">
</picture>

## How it works

### Storing multiple component variants with custom archetypes

A standard archetype like `Points3D` assigns a fixed column name, as well as component type & archetype metainformation, to every component it logs (`Points3D:colors`, `Points3D:positions`, etc.).
To store additional variants of a component on the same entity, log them as **custom archetypes**. The easiest way to do this is to use the `DynamicArchetype` utility.

snippet: howto/dual_color_point_cloud[log_custom_archetypes]

After this call, the entity's component list contains four components:
- `Points3D:positions`, `Points3D:radii` — from the standard archetype
- `HeightColors:colors` — the first custom variant
- `SpinColors:colors` — the second custom variant

You can inspect this in the viewer by selecting the entity in the streams panel:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/768w.png">
</picture>

### Configuring component mappings in blueprint

To make a visualizer read from an arbitrary source, we need to explicitly set the **component mapping** for a visualizer.
The mapping specifies a *target* (the component the visualizer expects) and a *source* (the component to actually read from).
Everything that isn't explicitly mapped keeps its default behavior.

snippet: howto/dual_color_point_cloud[blueprint]

In this example, each view overrides only the color source for the `Points3D` visualizer - positions, radii, and everything else are still read from their default sources automatically.

In the viewer you can access and this in the visualizer settings, presented when selecting an entity in a view:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/1200w.png">
</picture>


### Configuring component mappings in the UI

You can set up the same mappings interactively without writing any blueprint code:

- Add two 3D views (or clone an existing one)
- **Select the entity** in one of the views.
- In the selection panel, find the visualizer section (e.g. **Points3D**).
- Click on the component row you want to remap (e.g. **colors**) to expand the component mapping options.
- Change the source from the default to the desired custom archetype component (e.g. `HeightColors:colors`).
