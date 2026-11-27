---
title: Send user-defined data
order: 500
description: Log your own data types with AnyValues and DynamicArchetype
---

Rerun comes with many pre-built [Types](../../reference/types.md) that you can use out of the box. As long as your own data can be decomposed into Rerun [components](../../reference/types/components.md) or can be serialized with [Apache Arrow](https://arrow.apache.org/), you can log it directly without needing to recompile Rerun.

For Python and Rust we have helpers for this, called `AnyValues`, allowing you to easily attach custom values to any entity.
For C++ a similar thing can be accomplished without the helpers.
You find the documentation for these helpers here:

-   [`AnyValues` in Python](https://ref.rerun.io/docs/python/main/rerun/#rerun.AnyValues)
-   [`AnyValues` in Rust](https://docs.rs/rerun/latest/rerun/struct.AnyValues.html)

snippet: tutorials/any_values

If your values should be grouped together and that grouping isn't referred to from many places that need to stay aligned we have a helper for this called `DynamicArchetype`, which adds some structural grouping to multiple values.
`DynamicArchetype` stores the values under an [archetype](../data-model/archetypes.md) name of your choice.
The `confidence` field of `MyArchetype` becomes the column `MyArchetype:confidence`.
`AnyValues` stores no archetype name, so the same field becomes the column `confidence`.

You find the documentation for these helpers here:

-   [`DynamicArchetype` in Python](https://ref.rerun.io/docs/python/main/rerun/#rerun.DynamicArchetype)
-   [`DynamicArchetype` in Rust](https://docs.rs/rerun/latest/rerun/struct.DynamicArchetype.html)

snippet: tutorials/dynamic_archetype

You can also make your own type loggable by implementing the `AsComponents` [Python protocol](https://ref.rerun.io/docs/python/stable/rerun/#rerun.AsComponents) or [Rust trait](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html), which means implementing the function, `as_component_batches()`.

## Logging custom data

Use `DynamicArchetype` to send data with custom component names alongside regular Rerun data.
Flat arrays and Arrow `StructArray`s are both supported.

The example below logs scalar and nested data to an entity named `/plot`.
This is how its components appear in the Viewer:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/full.png" alt="Data overview for the /plot entity showing scalar, custom, and nested components">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/1024w.png">
</picture>

snippet: howto/component_mapping[custom_data]

## Remapping to a Rerun archetype

Let's start with a simple example where you have your own point cloud class that is perfectly representable as a Rerun archetype.

```python
@dataclass
class LabeledPoints:
    points: np.ndarray
    labels: list[str]
```

If you implement `as_component_batches()` on `LabeledPoints`, you can pass it directly to `rr.log`. The simplest possible way is to use the matching Rerun archetype’s `as_component_batches` method.

snippet: tutorials/remap_to_archetype[labeled_points]

Then pass it to `rr.log` like any archetype:

snippet: tutorials/remap_to_archetype[log]

## Custom archetypes and components

You can also define and log your own custom archetypes and components completely from user code, without rebuilding Rerun.

In this example we extend the Rerun Points3D archetype with a custom confidence component and user-defined archetype.

This is what it looks like in the dataframe view:

snippet: tutorials/custom_data

<picture>
  <img src="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/full.png" alt="Dataframe view with Points3D positions and user.CustomPoints3D confidences columns for two custom point cloud entities">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/1200w.png">
</picture>

The point clouds still show up in a 3D view, because `CustomPoints3D` also logs the components of the built-in `Points3D`.


## Creating/augmenting visualizations from custom data

All components can be mapped to arbitrary slots of visualizers.
For general information on component mapping see [component mappings](../../viewer/view-concepts/component-mappings.md),
for the common case of plotting see [plot any scalar](../../viewer/view-types/time-series.md#plot-any-scalar)

> [!NOTE]
> The Viewer does not support complex mappings, e.g. from scalars to colors.
> To reshape data like this, apply [Lenses](../../query-and-transform/lenses.md) in the SDK before the data reaches the Viewer.
