---
title: Remap your data
order: 200
description: Visualize custom messages and existing components in a view using component mappings
keywords: ROS 2, MCAP, Protobuf, custom messages, selector
---

Remapping makes a visualizer read an existing component or nested field that was not logged for that visualizer.
The recording blueprint setting is called a **component mapping**.
For example, plot a custom numeric field as a time series without changing the recording.
The MCAP importer can expose custom ROS 2 and Protobuf fields using [schema reflection](../../ingest/mcap/message-formats.md#schema-reflection).

## What remapping is

Visualizers can source their components from other data logged on the same entity, even data with arbitrary semantics.

Within a view, a visualizer can pick up any component that has the same datatype as the builtin type that it expects.
For example, the `SeriesLines` and `SeriesPoints` visualizers can pick up any numerical data for their [`Scalar`](../../reference/types/components/scalar.md) component.
The same holds for String-like components that can be selected for [`Name`](../../reference/types/components/name.md).
Likewise, the state timeline view's visualizer can source its `StateChange:state` input from any string, boolean, or numeric component (see [Visualize state changes](../view-types/state-timeline.md)).

Such data often comes from MCAP data that has user-defined message types, or from components that were flexibly logged via [`AnyValues`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues) or [`DynamicArchetype`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.DynamicArchetype).
The Viewer can even look for data with compatible datatypes in nested fields of Arrow [`StructArrays`](https://docs.rs/arrow/latest/arrow/array/struct.StructArray.html).

This applies to required components too, not only to optional ones: you can reinterpret a mesh as a point cloud, or graph node positions as plain 2D points.
Two kinds of component are excluded.
Enum components such as [`FillMode`](../../reference/types/components/fill_mode.md) only accept data that carries the matching Rerun semantics.
Image-like visualizers take an arbitrary buffer as their pixel data, but still require a proper [`ImageFormat`](../../reference/types/components/image_format.md) alongside it.

## Remap in the Viewer

Select an entity in a view, expand the visualizer input in the selection panel, and choose another compatible component in its source menu.
You can also drag a compatible component from the streams panel onto a time series or state timeline view to add a visualizer.

Suitable components show up in the source dropdown:

<picture>
  <img src="https://static.rerun.io/source_dropdown_scalars/9fb672d6984475010dfb58df485281c327f80368/full.png" alt="Source dropdown for the scalars of a SeriesLines visualizer, listing JointState joint_positions, velocities, efforts and timestamp fields">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/source_dropdown_scalars/9fb672d6984475010dfb58df485281c327f80368/480w.png">
</picture>




For a time series view, drag a compatible numeric component from the streams panel onto the view to add a `SeriesLines` visualizer.
For a state timeline view, drag a compatible string, boolean, or numeric component onto the view to add a lane.
Incompatible components and components already visualized are rejected.

## Remap from code

Set the visualizer input in a recording blueprint to point to another component:

snippet: howto/component_mapping[source_mapping]


## Example: two point cloud colorings

You can find the full example here:

* 🐍 [Python](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/dual_color_point_cloud.py)
* 🦀 [Rust](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/dual_color_point_cloud.rs)

<picture>
  <img src="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/full.png" alt="Two 3D views of the same torus point cloud side by side: Height Colors in a red, white and blue map and Spin Colors in a rainbow map">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mappings-viewer-overview/bb3c3d249b44f1ef7bd569f58481d146380b061d/1200w.png">
</picture>

### How the example works

#### Storing multiple component variants with custom archetypes

A standard archetype like `Points3D` assigns a fixed column name, as well as component type & archetype metainformation, to every component it logs (`Points3D:colors`, `Points3D:positions`, etc.).
To store additional variants of a component on the same entity, log them as **custom archetypes**.
The easiest way to do this is to use the `DynamicArchetype` utility.

snippet: howto/dual_color_point_cloud[log_custom_archetypes]

After this call, the entity's component list contains four components:
- `Points3D:positions`, `Points3D:radii` — from the standard archetype
- `HeightColors:colors` — the first custom variant
- `SpinColors:colors` — the second custom variant

You can inspect this in the viewer by selecting the entity in the streams panel:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/full.png" alt="Selection panel for the /pointcloud entity listing HeightColors and SpinColors color components next to Points3D positions and radii">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-selection/92c625844b0ad513c5985e71b825ee7121134959/768w.png">
</picture>

#### Configuring component mappings in blueprint

To make a visualizer read from an arbitrary source, we need to explicitly set the **component mapping** for a visualizer.
The mapping specifies a *target* (the component the visualizer expects) and a *source* (the component to actually read from).
Everything that isn't explicitly mapped keeps its default behavior.

snippet: howto/dual_color_point_cloud[blueprint]

In this example, each view overrides only the color source for the `Points3D` visualizer — positions, radii, and everything else are still read from their default sources automatically.

In the viewer you can access this in the visualizer settings, presented when selecting an entity in a view:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/full.png" alt="Source dropdown for the Points3D colors component, with SpinColors:colors selected over HeightColors:colors, View default and Add custom">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mappings-source-mapping/5718b252fca23f80f794bdd51a4eb391e0466fe6/1200w.png">
</picture>


#### Configuring component mappings in the UI

You can set up the same mappings interactively without writing any blueprint code:

- Add two 3D views (or clone an existing one)
- **Select the entity** in one of the views.
- In the selection panel, find the visualizer section (e.g. **Points3D**).
- Click on the component row you want to remap (e.g. **colors**) to expand the component mapping options.
- Change the source from the default to the desired custom archetype component (e.g. `HeightColors:colors`).

## Select nested fields

See [Selector syntax](../../query-and-transform/lenses.md#selectors) for the shared selector language.

When your data lives inside an Arrow `StructArray`, use a _selector_ to extract a specific field.
Selectors use a `jq`-inspired syntax (e.g. `.values` to select the `values` field).

Data types are automatically cast when compatible.
For example, `Float32` data will be cast to `Float64` as needed by the visualizer.

Here is how to create a nested `StructArray`:

snippet: howto/component_mapping[nested_struct]

The following remaps the `Scalars:scalars` input to read from `custom:my_nested_scalar` and selects the `values` field:

snippet: howto/component_mapping[selector_mapping]


## More examples

- [Plot any scalar](../view-types/time-series.md#plot-any-scalar)
- [Visualize any component as state](../view-types/state-timeline.md#visualize-any-component-as-state)
- [Visualize custom message scalars from an MCAP](../../ingest/mcap/message-formats.md#example-time-series-plot-for-custom-message-scalars)

## Lens or remapping?

A component mapping selects an existing component on the same entity, or a nested field, when one visualizer reads data in one view.
It is stored in the recording blueprint and does not change the recording.
Change the mapping to change that view without affecting other views or queries.

A [lens](../../query-and-transform/lenses.md) transforms chunks: it can compute values, create component columns, move data to another entity, or create timelines.
When used during ingestion, the transformed data can be stored so views and queries see it; lenses can also transform chunks read from existing data.
Changing a stored transformation requires processing the data again.
Both features use selectors to address fields.

| | Component mapping | Lens |
| --- | --- | --- |
| When | When a view reads data | When chunks are processed |
| Where | Recording blueprint or Viewer | SDK, importer, or chunk stream |
| Effect | Changes what one visualizer reads | Transforms data for downstream consumers |
| Scope | A component on the same entity or a nested field | Can create or move data across entities and timelines |
| Computation | Select a field or cast a compatible type | Compute and transform values |


If the value already exists and only one view needs it, remap it.
If it needs to be computed or moved, or other consumers need the transformed data, use a lens.
For a fixed visual style such as a color, use a [visualizer override](customize-views.md#set-a-value-for-one-entity).
