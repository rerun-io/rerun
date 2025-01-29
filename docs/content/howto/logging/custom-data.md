---
title: Send user-defined data
order: 200
description: How to use Rerun with custom data
---
Rerun comes with many pre-built [Types](../../reference/types.md) that you can use out of the box. As long as your own data can be decomposed into Rerun [components](../../reference/types/components.md) or can be serialized with [Apache Arrow](https://arrow.apache.org/), you can log it directly without needing to recompile Rerun.

For Python we have a helper for this, called [`AnyValues`](https://ref.rerun.io/docs/python/main/common/custom_data/), allowing you to easily attach custom values to any entity instance:

```
rr.log(
    "my_entity", rr.AnyValues(
        confidence=[1.2, 3.4, 5.6],
        description="Bla bla bla…",
    ),
)
```

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

In this example we extend the Rerun Points3D archetype with custom confidence and indicator components.

snippet: tutorials/custom_data

<picture>
  <img src="https://static.rerun.io/custom_data/7bb90e1ab4244541164775473c5106e15152b8d0/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_data/7bb90e1ab4244541164775473c5106e15152b8d0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_data/7bb90e1ab4244541164775473c5106e15152b8d0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_data/7bb90e1ab4244541164775473c5106e15152b8d0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_data/7bb90e1ab4244541164775473c5106e15152b8d0/1200w.png">
</picture>
