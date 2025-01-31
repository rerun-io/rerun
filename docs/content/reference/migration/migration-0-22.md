---
title: Migrating from 0.21 to 0.22
order: 988
---

### Previously deprecated `DisconnectedSpace` archetype/component got now removed

The deprecated `DisconnectedSpace` archetype and `DisconnectedSpace` component have been removed.
To achieve the same effect, you can log any of the following "invalid" transforms:
* zeroed 3x3 matrix
* zero scale
* zeroed quaternion
* zero axis on axis-angle rotation

Previously, the `DisconnectedSpace` archetype played a double role by governing view spawn heuristics & being used as a transform placeholder.
This led to a lot of complexity and often broke or caused confusion (see https://github.com/rerun-io/rerun/issues/6817, https://github.com/rerun-io/rerun/issues/4465, https://github.com/rerun-io/rerun/issues/4221).
By now, explicit blueprints offer a better way to express which views should be spawned and what content they should query.
(you can learn more about blueprints [here](https://rerun.io/docs/getting-started/configure-the-viewer/through-code-tutorial)).


### Removed `num_instances` keyword argument to `rr.log_components()`

For historical reasons, the `rr.log_components()` function of the Python SDK accepts an optional, keyword-only argument `num_instances`.
It was no longer used for several releases, so we removed it.

**Note**: although `rr.log_components()` is technically a public API, it is undocumented, and we discourage using it.
For logging custom components, use [`rr.AnyValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues) and [`rr.AnyBatchValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyBatchValue).


### Rust's `ViewCoordinates` archetype now has static methods instead of constants

As part of the switch to "eager archetype serialization" (serialization of archetype components now occurs at time of archetype instantiation rather than logging), we can no longer offer constants for the `ViewCoordinates` archetype like `ViewCoordinates::RUB`.

Instead, there's now methods with the same name, i.e. `ViewCoordinates::RUB()`.

### Rust's `Tensor` archetype can no longer access tensor data as `ndarray` view directly

As part of the switch to "eager archetype serialization" (serialization of archetype components now occurs at time of archetype instantiation rather than logging), we can no longer offer exposing the `Tensor` **archetype** as `ndarray::ArrayView` directly.

However, it is still possible to do so with the `TensorData` component.

### C++ `RecordingStream::log`/`send_column` no longer takes raw component collections

Previously, both `RecordingStream::log` and `RecordingStream::send_column` were able to
handle raw component collections which then would be serialized to arrow on the fly.

#### `log`

Under the hood we allow any type that implements the `AsComponents` trait.
However, `AsComponents` is no longer implemented for collections of components / implementors of `Loggable`.

Instead, you're encouraged to use archetypes for cases where you'd previously use loose collections of components.
This is made easier by the fact that archetypes can now be created without specifying required components.
For example, colors of a point cloud can be logged without position data:

```cpp
rec.log("points", rerun::Points3D().with_colors(colors));
```

Custom implementations of `AsComponents` still work as before.

#### `send_column`

Only `rerun::ComponentColumn` and anything else from which
a `Collection<ComponentColumn>` can be constructed is accepted.
The preferred way to create `rerun::ComponentColumn`s is to use the new `columns` method on archetypes.

For instance in order to send a column of scalars, you can now do this.
```cpp
rec.send_columns("scalars", time_column,
    rerun::Scalar().with_many_scalar(scalar_data).columns()
);
```
All [example snippets](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/INDEX.md?speculative-link) have been updated accordingly.


### C++ `AsComponents::serialize` is now called `AsComponents::as_batches` and returns `rerun::Collection<ComponentBatch>`

The `AsComponents`'s `serialize` method has been renamed to `as_batches` and now returns a `rerun::Collection<ComponentBatch>` instead of a `std::vector<ComponentBatch>`.

```cpp
// Old
template <>
struct AsComponents<CustomArchetype> {
    static Result<std::vector<ComponentBatch>> serialize(const CustomArchetype& archetype);
};

// New
template <>
struct AsComponents<CustomArchetype> {
    static Result<rerun::Collection<ComponentBatch>> operator()(const CustomArchetype& archetype);
};
```
