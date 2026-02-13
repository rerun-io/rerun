---
title: Migrating from 0.21 to 0.22
order: 988
---

## Major changes to the logging APIs


### Partial updates

These new APIs make it possible to send partial updates of your data over time, i.e. you can think of this as a sort of diffs or delta encodings.

This was already possible before, but only by relying on semi-private APIs that were not without their lot of issues.\
In particular, these APIs had no way of keeping track of the surrounding context in which these logging calls were made (e.g. which archetype?), which created a lot of data modeling related issues.\
Internally, these new APIs make it possible to implement many long awaited Rerun features, in the long term.

The following snippets give a succinct before/after picture; for more information about partial updates, please [refer to the dedicated documentation](../../howto/logging-and-ingestion/send-partial-updates.md).


#### Python

*Before*:
```python
positions = [[i, 0, 0] for i in range(0, 10)]

rr.set_time_sequence("frame", 0)
rr.log("points", rr.Points3D(positions))

for i in range(0, 10):
    colors = [[20, 200, 20] if n < i else [200, 20, 20] for n in range(0, 10)]
    radii = [0.6 if n < i else 0.2 for n in range(0, 10)]

    # Update only the colors and radii, leaving everything else as-is.
    rr.set_time_sequence("frame", i)
    rr.log("points", [rr.components.ColorBatch(colors), rr.components.RadiusBatch(radii)])

# Update the positions and radii, and clear everything else in the process.
rr.set_time_sequence("frame", 20)
rr.log("points", rr.Clear.flat())
rr.log("points", [rr.components.Position3DBatch(positions), rr.components.RadiusBatch(0.3)])
```

*After*:
```python
positions = [[i, 0, 0] for i in range(0, 10)]

rr.set_time_sequence("frame", 0)
rr.log("points", rr.Points3D(positions))

for i in range(0, 10):
    colors = [[20, 200, 20] if n < i else [200, 20, 20] for n in range(0, 10)]
    radii = [0.6 if n < i else 0.2 for n in range(0, 10)]

    # Update only the colors and radii, leaving everything else as-is.
    rr.set_time_sequence("frame", i)
    rr.log("points", rr.Points3D.from_fields(radii=radii, colors=colors))

# Update only the colors and radii, leaving everything else as-is.
rr.set_time_sequence("frame", 20)
rr.log("points", rr.Points3D.from_fields(clear_unset=True, positions=positions, radii=0.3))
```

See also:
* [Example: Partial updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_partial_updates.py)
* [Example: Partial updates of a `Mesh3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/mesh3d_partial_updates.py)


#### Rust

*Before*:
```rust
let positions = || (0..10).map(|i| (i as f32, 0.0, 0.0)).map(Into::into).collect::<Vec<rerun::components::Position3D>>();

rec.set_time_sequence("frame", 0);
rec.log("points", &rerun::Points3D::new(positions()))?;

for i in 0..10 {
    let colors: Vec<rerun::components::Color> = (0..10)
        .map(|n| { if n < i { rerun::Color::from_rgb(20, 200, 20) } else { rerun::Color::from_rgb(200, 20, 20) } })
        .collect();
    let radii: Vec<rerun::components::Radius> = (0..10)
        .map(|n| if n < i { 0.6 } else { 0.2 })
        .map(Into::into)
        .collect();

    // Update only the colors and radii, leaving everything else as-is.
    rec.set_time_sequence("frame", i);
    rec.log("points", &[&radii as &dyn rerun::ComponentBatch, &colors])?;
}

// Update the positions and radii, and clear everything else in the process.
let radii: Vec<rerun::components::Radius> = vec![0.3.into()];
rec.set_time_sequence("frame", 20);
rec.log("points", &rerun::Clear::flat())?;
rec.log("points", &[&positions() as &dyn rerun::ComponentBatch, &radii])?;
```


*After*:
```rust
let positions = || (0..10).map(|i| (i as f32, 0.0, 0.0));

rec.set_time_sequence("frame", 0);
rec.log("points", &rerun::Points3D::new(positions()))?;

for i in 0..10 {
    let colors = (0..10).map(|n| { if n < i { rerun::Color::from_rgb(20, 200, 20) } else { rerun::Color::from_rgb(200, 20, 20) } });
    let radii = (0..10).map(|n| if n < i { 0.6 } else { 0.2 });

    // Update only the colors and radii, leaving everything else as-is.
    rec.set_time_sequence("frame", i);
    rec.log("points", &rerun::Points3D::update_fields().with_radii(radii).with_colors(colors))?;
}

// Update the positions and radii, and clear everything else in the process.
rec.set_time_sequence("frame", 20);
rec.log("points", &rerun::Points3D::clear_fields().with_positions(positions()).with_radii([0.3]))?;
```

See also:
* [Example: Partial updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_partial_updates.rs)
* [Example: Partial updates of a `Mesh3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/mesh3d_partial_updates.rs)


#### C++

*Before*:
```cpp
std::vector<rerun::Position3D> positions;
for (int i = 0; i < 10; ++i) {
    positions.emplace_back(static_cast<float>(i), 0.0f, 0.0f);
}

rec.set_time_sequence("frame", 0);
rec.log("points", rerun::Points3D(positions));

for (int i = 0; i < 10; ++i) {
    std::vector<rerun::Color> colors;
    for (int n = 0; n < 10; ++n) {
        if (n < i) { colors.emplace_back(rerun::Color(20, 200, 20)); } else { colors.emplace_back(rerun::Color(200, 20, 20)); }
    }

    std::vector<rerun::Radius> radii;
    for (int n = 0; n < 10; ++n) {
        if (n < i) { radii.emplace_back(rerun::Radius(0.6f)); } else { radii.emplace_back(rerun::Radius(0.2f)); }
    }

    // Update only the colors and radii, leaving everything else as-is.
    rec.set_time_sequence("frame", i);
    rec.log("points", colors, radii);
}

std::vector<rerun::Radius> radii;
radii.emplace_back(0.3f);

// Update the positions and radii, and clear everything else in the process.
rec.set_time_sequence("frame", 20);
rec.log("points", rerun::Clear::FLAT);
rec.log("points", positions, radii);
```


*After*:
```cpp
std::vector<rerun::Position3D> positions;
for (int i = 0; i < 10; ++i) positions.emplace_back(static_cast<float>(i), 0.0f, 0.0f);

rec.set_time_sequence("frame", 0);
rec.log("points", rerun::Points3D(positions));

for (int i = 0; i < 10; ++i) {
    std::vector<rerun::Color> colors;
    for (int n = 0; n < 10; ++n) {
        if (n < i) { colors.emplace_back(rerun::Color(20, 200, 20)); } else { colors.emplace_back(rerun::Color(200, 20, 20)); }
    }

    std::vector<rerun::Radius> radii;
    for (int n = 0; n < 10; ++n) {
        if (n < i) { radii.emplace_back(rerun::Radius(0.6f)); } else { radii.emplace_back(rerun::Radius(0.2f)); }
    }

    // Update only the colors and radii, leaving everything else as-is.
    rec.set_time_sequence("frame", i);
    rec.log("points", rerun::Points3D::update_fields().with_radii(radii).with_colors(colors));
}

std::vector<rerun::Radius> radii;
radii.emplace_back(0.3f);

// Update the positions and radii, and clear everything else in the process.
rec.set_time_sequence("frame", 20);
rec.log("points", rerun::Points3D::clear_fields().with_positions(positions).with_radii(radii));
```

See also:
* [Example: Partial updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_partial_updates.cpp)
* [Example: Partial updates of a `Mesh3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/mesh3d_partial_updates.cpp)


### Columnar updates

These new APIs make it possible to send partial updates of your data over time, i.e. you can think of this as a sort of diffs or delta encodings.

This was already possible before, although with pretty severe limitations.\
In particular, these APIs had no way of keeping track of the surrounding context in which these logging calls were made (e.g. which archetype?), which created a lot of data modeling related issues.\
Internally, these new APIs make it possible to implement many long awaited Rerun features, in the long term.

The following snippets give a succinct before/after picture; for more information about partial updates, please [refer to the dedicated documentation](http://rerun.io/docs/howto/logging/send-columns).

See also the API reference:
* [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#ad17571d51185ce2fc2fc2f5c3070ad65)
* [🐍 Python](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_columns)
* [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.send_columns)

#### Python

*Before*:
```python
# Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
times = np.arange(10, 15, 1.0)
positions = [
    [[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
    [[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
    [[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
    [[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
    [[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
]
positions_arr = np.concatenate(positions)

# At each timestep, all points in the cloud share the same but changing color and radius.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
radii = [0.05, 0.01, 0.2, 0.1, 0.3]

rr.send_columns(
    "points",
    indexes=[rr.TimeSecondsColumn("time", times)],
    components=[
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions_arr).partition([len(row) for row in positions]),
        rr.components.ColorBatch(colors),
        rr.components.RadiusBatch(radii),
    ],
)
```

*After*:
```python
# Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
times = np.arange(10, 15, 1.0)
# fmt: off
positions = [
    [1.0, 0.0, 1.0], [0.5, 0.5, 2.0],
    [1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0],
    [2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5],
    [-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5],
    [1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0],
]
# fmt: on

# At each timestep, all points in the cloud share the same but changing color and radius.
colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
radii = [0.05, 0.01, 0.2, 0.1, 0.3]

rr.send_columns(
    "points",
    indexes=[rr.TimeSecondsColumn("time", times)],
    columns=[
        *rr.Points3D.columns(positions=positions).partition(lengths=[2, 4, 4, 3, 4]),
        *rr.Points3D.columns(colors=colors, radii=radii),
    ],
)
```

See also:
* [Example: Columnar updates of a `Scalar` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/scalar_column_updates.py)
* [Example: Columnar updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_column_updates.py)
* [Example: Columnar updates of an `Image` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/image_column_updates.py)


#### Rust

*Before*: N/A. This was not previously possible using the Rust API.

*After*:
```rust
let times = rerun::TimeColumn::new_seconds("time", 10..15);

// Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
#[rustfmt::skip]
let positions = [
    [1.0, 0.0, 1.0], [0.5, 0.5, 2.0],
    [1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0],
    [2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5],
    [-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5],
    [1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0],
];

// At each timestep, all points in the cloud share the same but changing color and radius.
let colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF];
let radii = [0.05, 0.01, 0.2, 0.1, 0.3];

// Partition our data as expected across the 5 timesteps.
let position = rerun::Points3D::update_fields().with_positions(positions).columns([2, 4, 4, 3, 4])?;
let color_and_radius = rerun::Points3D::update_fields().with_colors(colors).with_radii(radii).columns_of_unit_batches()?;

rec.send_columns("points", [times], position.chain(color_and_radius))?;
```

See also:
* [Example: Columnar updates of a `Scalar` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/scalar_column_updates.rs)
* [Example: Columnar updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_column_updates.rs)
* [Example: Columnar updates of an `Image` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/image_column_updates.rs)


#### C++

*Before*:
```cpp
// Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
std::vector<std::array<float, 3>> positions = {
    // clang-format off
    {1.0, 0.0, 1.0}, {0.5, 0.5, 2.0},
    {1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0},
    {2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5},
    {-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5},
    {1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0},
    // clang-format on
};

// At each timestep, all points in the cloud share the same but changing color and radius.
std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};

// Log at seconds 10-14
auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
auto time_column = rerun::TimeColumn::from_times("time", std::move(times));

// Partition our data as expected across the 5 timesteps.
auto indicator_batch = rerun::ComponentColumn::from_indicators<rerun::Points3D>(5);
auto position_batch = rerun::ComponentColumn::from_loggable_with_lengths(
    rerun::Collection<rerun::components::Position3D>(std::move(positions)),
    {2, 4, 4, 3, 4}
);
auto color_batch = rerun::ComponentColumn::from_loggable(
    rerun::Collection<rerun::components::Color>(std::move(colors))
);
auto radius_batch = rerun::ComponentColumn::from_loggable(
    rerun::Collection<rerun::components::Radius>(std::move(radii))
);

rec.send_columns(
    "points",
    time_column,
    {
        indicator_batch.value_or_throw(),
        position_batch.value_or_throw(),
        color_batch.value_or_throw(),
        radius_batch.value_or_throw(),
    }
);
```

*After*:
```cpp
// Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
std::vector<std::array<float, 3>> positions = {
    // clang-format off
    {1.0, 0.0, 1.0}, {0.5, 0.5, 2.0},
    {1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0},
    {2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5},
    {-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5},
    {1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0},
    // clang-format on
};

// At each timestep, all points in the cloud share the same but changing color and radius.
std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};

// Log at seconds 10-14
auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
auto time_column = rerun::TimeColumn::from_times("time", std::move(times));

// Partition our data as expected across the 5 timesteps.
auto position = rerun::Points3D().with_positions(positions).columns({2, 4, 4, 3, 4});
auto color_and_radius = rerun::Points3D().with_colors(colors).with_radii(radii).columns();

rec.send_columns("points", time_column, position, color_and_radius);
```

See also:
* [Example: Columnar updates of a `Scalar` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/scalar_column_updates.cpp)
* [Example: Columnar updates of a `Transform3D` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/transform3d_column_updates.cpp)
* [Example: Columnar updates of an `Image` archetype](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/all/archetypes/image_column_updates.cpp)



## Rust API changes

### `ViewCoordinates` archetype now has static methods instead of constants

As part of the switch to "eager archetype serialization" (serialization of archetype components now occurs at time of archetype instantiation rather than logging), we can no longer offer constants for the `ViewCoordinates` archetype like `ViewCoordinates::RUB`.

Instead, there's now methods with the same name, i.e. `ViewCoordinates::RUB()`.


### `Tensor` archetype can no longer access tensor data as `ndarray` view directly

As part of the switch to "eager archetype serialization" (serialization of archetype components now occurs at time of archetype instantiation rather than logging), we can no longer offer exposing the `Tensor` **archetype** as `ndarray::ArrayView` directly.

However, it is still possible to do so with the `TensorData` component.

### Default log level changed to `warn` in `re_log`

With the addition of the notification center, the default log level in `re_log` is now set to `warn`.

Logs at the `info` level will appear in the notification center.


## C++ API changes

### `RecordingStream::log`/`send_column` no longer takes raw component collections

Previously, both `RecordingStream::log` and `RecordingStream::send_column` were able to
handle raw component collections which then would be serialized to arrow on the fly.


#### `log`

Under the hood we allow any type that implements the `AsComponents` trait.
However, `AsComponents` is no longer implemented for collections of components / implementers of `Loggable`.

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
All [example snippets](https://github.com/rerun-io/rerun/blob/0.22.0/docs/snippets/INDEX.md) have been updated accordingly.


## `AsComponents::serialize` is now called `AsComponents::as_batches` and returns `rerun::Collection<ComponentBatch>`

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

## Python API changes

### `rr.log_components()` is now deprecated & no longer has a `num_instances` keyword argument

For historical reasons, the `rr.log_components()` function of the Python SDK accepts an optional, keyword-only argument `num_instances`.
It was no longer used for several releases, so we removed it.

Although `rr.log_components()` was technically a public API, it was undocumented and we now deprecated its use.
For logging custom components, use [`rr.AnyValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues) and [`rr.AnyBatchValue`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyBatchValue).


## Other

### Previously deprecated `DisconnectedSpace` archetype/component have been removed

The deprecated `DisconnectedSpace` archetype and `DisconnectedSpace` component have been removed.
To achieve the same effect, you can log any of the following "invalid" transforms:
* zeroed 3x3 matrix
* zero scale
* zeroed quaternion
* zero axis on axis-angle rotation

Previously, the `DisconnectedSpace` archetype played a double role by governing view spawn heuristics & being used as a transform placeholder.
This led to a lot of complexity and often broke or caused confusion (see https://github.com/rerun-io/rerun/issues/6817, https://github.com/rerun-io/rerun/issues/4465, https://github.com/rerun-io/rerun/issues/4221).
By now, explicit blueprints offer a better way to express which views should be spawned and what content they should query.
(you can learn more about blueprints [here](../../getting-started/configure-the-viewer#programmatic-blueprints)).
