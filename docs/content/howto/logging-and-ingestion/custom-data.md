---
title: Send user-defined data
order: 500
description: How to use Rerun with custom data
---

Rerun comes with many pre-built [Types](../../reference/types.md) that you can use out of the box. As long as your own data can be decomposed into Rerun [components](../../reference/types/components.md) or can be serialized with [Apache Arrow](https://arrow.apache.org/), you can log it directly without needing to recompile Rerun.

For Python and Rust we have helpers for this, called `AnyValues`, allowing you to easily attach custom values to any entity instance.
For C++ a similar thing can be accomplished without the helpers.
You find the documentation for these helpers here:

-   [`AnyValues` in Python](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues)
-   [`AnyValues` in Rust](https://docs.rs/rerun/latest/rerun/struct.AnyValues.html)

snippet: tutorials/any_values

If your values should be grouped together and that grouping isn't referred to from many places that need to stay aligned we have a helpers for this called, `DynamicArchetype` which adds some structural grouping to multiple values.

You find the documentation for these helpers here:

-   [`DynamicArchetype` in Python](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.DyanamicArchetype)
-   [`DynamicArchetype` in Rust](https://docs.rs/rerun/latest/rerun/struct.DynamicArchetype.html)

snippet: tutorials/dynamic_archetype

You can also create your own component by implementing the `AsComponents` [Python protocol](https://ref.rerun.io/docs/python/0.9.0/common/interfaces/#rerun.AsComponents) or [Rust trait](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html), which means implementing the function, `as_component_batches()`.

## Remapping to a Rerun archetype

Let's start with a simple example where you have your own point cloud class that is perfectly representable as a Rerun archetype.

```python
@dataclass
class LabeledPoints:
    points: np.ndarray
    labels: List[str])
```

If you implement `as_component_batches()` on `LabeledPoints`, you can pass it directly to `rr.log`. The simplest possible way is to use the matching Rerun archetype’s `as_component_batches` method.

```python
import rerun as rr  # pip install rerun-sdk

@dataclass
class LabeledPoints:
    points: np.ndarray
    labels: List[str]

    def as_component_batches(self) -> list[rr.ComponentBatch]:
        return rr.Points3D(positions=self.points,
                           labels=self.labels).as_component_batches()
…
# Somewhere deep in your code
classified = my_points_classifier(…)  # type: LabeledPoints
rr.log("points/classified", classified)
```

## Custom archetypes and components

You can also define and log your own custom archetypes and components completely from user code, without rebuilding Rerun.

In this example we extend the Rerun Points3D archetype with a custom confidence component and user-defined archetype.

⚠️ NOTE: Due to the component descriptor changes in `v0.24` it is currently not possible for custom data to be picked up by visualizers.
We are currently investigating approaches to bring that functionality back.

However, your custom data will still show up in the dataframe view, as shown below.

snippet: tutorials/custom_data

<picture>
  <img src="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_data_dataframe/16d49401a8c9ed40d948623a8f4188104e4bfb64/1200w.png">
</picture>
