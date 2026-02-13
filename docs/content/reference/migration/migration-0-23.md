---
title: Migrating from 0.22 to 0.23
order: 987
---

## Reserved namespaces
Starting with this release, the SDKs will log Rerun-related information to reserved entity path namespaces that are prefixed with `__`.
Most notably, there is `__warnings/`, which used to be called `rerun/` and can be used to log exceptions that occurred in the SDKs.
We also introduced `__properties/`, which stores recording-level information that is logged via the new `set_property` methods in the SDKs.
Reserved namespaces are highlighted with a ⚙️ icon in the viewer UI.

## Timelines are uniquely identified by name
Previously, you could (confusingly) have two timelines with the same name, as long as they had different types (sequence vs temporal).
This is no longer possible.
Timelines are now uniquely identified by name, and if you use different types on the same timeline, you will get a logged warning, and the _latest_ type will be used to interpret the full set of time data.

## Unify the names of time units
We have been wildly inconsistent with how we name our time units, and it is time we fixed it. So starting now, we're using:

* `secs` instead of `s` or `seconds`
* `nanos` instead of `ns` or `nanoseconds`
* `millis` instead of `ms` or `milliseconds`

All function and parameters using the old names have been deprecated, and will be removed in a future version.

##### Why these names?
* They are short without being cryptic
* They are the ones the Rust standard library (mostly) use: https://doc.rust-lang.org/stable/std/time/struct.Duration.html
* Anything is better than being inconsistent :)

## Differentiate between timestamps and durations
We've added a explicit API for setting time, where you need to explicitly specify if a time is either a timestamp (e.g. `2025-03-03T14:34:56.123456789`) or a duration (e.g. `123s`).

Before, Rerun would try to guess what you meant (small values were assumed to be durations, and large values were assumes to be durations since the Unix epoch, i.e. timestamps).
Now you need to be explicit.


### 🦀 Rust: deprecated `RecordingStream::set_time_secs` and `set_time_nanos`
Use one of these instead:
* `set_duration_secs`
* `set_timestamp_secs_since_epoch`
* `set_time` with `std::time::Duration`
* `set_time` with `std::time::SystemTime`


### 🌊 C++
We've deprecated the following functions, with the following replacements:
* `set_time` -> `set_time_duration` or `set_time_timestamp`
* `set_time_seconds` -> `set_time_duration_secs` or `set_time_timestamp_secs_since_epoch`
* `set_time_nanos` -> `set_time_duration_nanos` or `set_time_timestamp_nanos_since_epoch`

`TimeColumn` also has deprecated functions.


### 🐍 Python: replaced `rr.set_time_*` functions with a single `rr.set_time`
We've deprecated `rr.set_time_secs`, `rr.set_time_nanos`, as well as `rr.set_time_sequence` and replaced them with `rr.set_time`.
`set_time` takes either a `sequence=`, `duration=` or `timestamp=` argument.

`duration` must be either:
* seconds as `int` or `float`
* [`datetime.timedelta`](https://docs.python.org/3/library/datetime.html#datetime.timedelta)
* [`numpy.timedelta64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.timedelta64)

`timestamp` must be either:
* seconds since unix epoch (1970-01-01) as `int` or `float`
* [`datetime.datetime`](https://docs.python.org/3/library/datetime.html#datetime.datetime)
* [`numpy.datetime64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.datetime64)

#### Migrating
##### `rr.set_sequence("foo", 42)`
New: `rr.set_time("foo", sequence=42)`

##### `rr.set_time_secs("foo", duration_secs)`
When using relative times (durations/timedeltas): `rr.set_time("foo", duration=duration_secs)`
You can also pass in a [`datetime.timedelta`](https://docs.python.org/3/library/datetime.html#datetime.timedelta) or [`numpy.timedelta64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.timedelta64) directly.

##### `rr.set_time_secs("foo", seconds_since_epoch)`
New: `rr.set_time("foo", timestamp=seconds_since_epoch)`
You can also pass in a [`datetime.datetime`](https://docs.python.org/3/library/datetime.html#datetime.datetime) or [`numpy.datetime64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.datetime64) directly.

##### `rr.set_time_nanos("foo", duration_nanos)`
Either:
* `rr.set_time("foo", duration=1e-9 * duration_nanos)`
* `rr.set_time("foo", duration=np.timedelta64(duration_nanos, 'ns'))`

The former is subject to (double-precision) floating point precision loss (but still nanosecond precision for timedeltas below less than 100 days in duration), while the latter is lossless.

##### `rr.set_time_nanos("foo", nanos_since_epoch)`
Either:
* `rr.set_time("foo", timestamp=1e-9 * nanos_since_epoch)`
* `rr.set_time("foo", timestamp=np.datetime64(nanos_since_epoch, 'ns'))`

The former is subject to (double-precision) floating point precision loss (still microsecond precision for the next century), while the latter is lossless.


### 🐍 Python: replaced `rr.Time*Column` with `rr.TimeColumn`
Similarly to the above new `set_time` API, there is also a new `TimeColumn` class that replaces `TimeSequenceColumn`, `TimeSecondsColumn`, and `TimeNanosColumn`.
The migration is very similar to the above.

#### Migration
##### `rr.TimeSequenceColumn("foo", values)`
New: `rr.TimeColumn("foo", sequence=values)`

##### `rr.TimeSecondsColumn("foo", duration_secs)`
New: `rr.TimeColumn("foo", duration=duration_secs)`

##### `rr.TimeSecondsColumn("foo", seconds_since_epoch)`
New: `rr.TimeColumn("foo", timestamp=seconds_since_epoch)`

##### `rr.TimeNanosColumn("foo", duration_nanos)`
Either:
* `rr.TimeColumn("foo", duration=1e-9 * duration_nanos)`
* `rr.TimeColumn("foo", duration=np.timedelta64(duration_nanos, 'ns'))`

The former is subject to (double-precision) floating point precision loss (but still nanosecond precision for timedeltas below less than 100 days in duration), while the latter is lossless.

##### `rr.TimeNanosColumn("foo", nanos_since_epoch)`
Either:
* `rr.TimeColumn("foo", duration=1e-9 * nanos_since_epoch)`
* `rr.TimeColumn("foo", duration=np.timedelta64(nanos_since_epoch, 'ns'))`

The former is subject to (double-precision) floating point precision loss (still microsecond precision for the next century), while the latter is lossless.


### Dataloader time arguments
The CLI API for external dataloaders has changed the following argument names:

* `--sequence` -> `--time_sequence`
* `--time` -> `--time_duration_nanos` or `--time_timestamp_nanos`


## 🐍 Python: `rr.new_recording` is now deprecated in favor of `rr.RecordingStream`

Previously, `RecordingStream` instances could be created with the `rr.new_recording()` function. This method is now deprecated in favor of directly using the [`RecordingStream`](https://ref.rerun.io/docs/python/0.23.0/common/initialization_functions/#rerun.RecordingStream) constructor. The `RecordingStream` constructor is mostly backward compatible, so in most case it is matter of using `RecordingStream` instead of `new_recording`:

<!-- NOLINT_START -->

```python
# before
rec = rr. new_recording("rerun_example")

# after
rec = rr.RecordingStream("my_app_id")
```

If you used the `spawn=True` argument, you will now have to call the `spawn()` method explicitly:

```python
# before
rec = rr. new_recording("my_app_id", spawn=True)

# after
rec = rr.RecordingStream("my_app_id")
rec.spawn()
```

<!-- NOLINT_END -->

## 🐍 Python: removed `rr.log_components()`, `rr.connect()`, `rr.connect_tcp()`, and `rr.serve()`

These functions were [deprecated](migration-0-22.md#python-api-changes) in 0.22 and are no longer available.

Calls to `rr.log_components()` API are now superseded by the new partial update API. See the [documentation](../../concepts/logging-and-ingestion/latest-at.md#partial-updates) and the [migration instructions](migration-0-22.md#partial-updates).

Calls to `rr.connect()` and `rr.connect_tcp()` must be changed to [`rr.connect_grpc()`](https://ref.rerun.io/docs/python/0.23.0/common/initialization_functions/#rerun.connect_grpc).

Calls to `rr.serve()` must be changed to [`rr.serve_web()`](https://ref.rerun.io/docs/python/0.23.0/common/initialization_functions/#rerun.serve_web).

## 🌊 C++: removed `connect` and `connect_tcp` from `RecordingStream`

Calls to these functions must be changed to `connect_grpc`. Note that the string passed to `connect_grpc` must now be a valid Rerun URL. If you were previously calling `connect_grpc("127.0.0.1:9876")`, it must be changed to `connect_grpc("rerun+http://127.0.0.1:9876/proxy")`.

See the [`RecordingStream` docs](https://ref.rerun.io/docs/cpp/0.23.0/classrerun_1_1RecordingStream.html) for more information.

## 🦀 Rust: removed `connect` and `connect_tcp` from `RecordingStream` and `RecordingStreamBuilder`

Calls to these functions must be changed to use [`connect_grpc`](https://docs.rs/rerun/0.23.0/rerun/struct.RecordingStreamBuilder.html#method.connect_grpc) instead.

Note that the string passed to `connect_grpc` must now be a valid Rerun URL. If you were previously calling `connect("127.0.0.1:9876")`, it must be changed to `connect_grpc("rerun+http://127.0.0.1:9876/proxy")`.

The following schemes are supported: `rerun+http://`, `rerun+https://` and `rerun://`, which is an alias for `rerun+https://`.
These schemes are then converted on the fly to either `http://` or `https://`.
Rerun uses gRPC-based protocols under the hood, which means that the paths (`/catalog`, `/recording/12345`, …) are mapped to gRPC services and methods on the fly.

## 🐍 Python: blueprint overrides & defaults are now archetype based

Just like with `send_columns` in the previous release, blueprint overrides and defaults are now archetype based.

**Examples:**

Setting default & override for radius

Before:
```python
rrb.Spatial2DView(
    name="Rect 1",
    origin="/",
    contents=["/**"],
    defaults=[rr.components.Radius(2)],
    overrides={"rect/0": [rr.components.Radius(1)]},
)
```
After:
```python
rrb.Spatial2DView(
    name="Rect 1",
    origin="/",
    contents=["/**"],
    defaults=[rr.Boxes2D.from_fields(radii=1)],
    overrides={"rect/0": rr.Boxes2D.from_fields(radii=2)},
)
```

Setting up styles for a plot.

Before:
```python
# …
rrb.TimeSeriesView(
    name="Trig",
    origin="/trig",
    overrides={
        "/trig/sin": [rr.components.Color([255, 0, 0]), rr.components.Name("sin(0.01t)")],
        "/trig/cos": [rr.components.Color([0, 255, 0]), rr.components.Name("cos(0.01t)")],
    },
),
rrb.TimeSeriesView(
    name="Classification",
    origin="/classification",
    overrides={
        "classification/line": [rr.components.Color([255, 255, 0]), rr.components.StrokeWidth(3.0)],
        "classification/samples": [rrb.VisualizerOverrides("SeriesPoints")], # This ensures that the `SeriesPoints` visualizers is used for this entity.
    },
),
# …
```
After:
```python
# …
rrb.TimeSeriesView(
    name="Trig",
    origin="/trig",
    overrides={
        "/trig/sin": rr.SeriesLines.from_fields(colors=[255, 0, 0], names="sin(0.01t)"),
        "/trig/cos": rr.SeriesLines.from_fields(colors=[0, 255, 0], names="cos(0.01t)"),
    },
),
rrb.TimeSeriesView(
    name="Classification",
    origin="/classification",
    overrides={
        "classification/line": rr.SeriesLines.from_fields(colors=[255, 255, 0], widths=3.0),
        "classification/samples": rrb.VisualizerOverrides("SeriesPoints"), # This ensures that the `SeriesPoints` visualizers is used for this entity.
    },
),
# …
```

⚠️ Warning: Just like regular log/send calls, overlapping component types still overwrite each other.
E.g. overriding a box radius will also override point radius on the same entity.
In a future release, components tagged with a different archetype or field name can live side by side,
but for the moment the Viewer is not able to make this distinction.
For details see [#6889](https://github.com/rerun-io/rerun/issues/6889).


### Overriding `Visible` and `Interactive` is now always recursive

Previously, it was possible to override visibility individually, but not recursively.
Also, Viewer interaction [was hampered](https://github.com/rerun-io/rerun/issues/9254) by this.

Overrides for these two properties are now always recursive, and can be applied using the new `EntityBehavior` archetype.

Before:
```python
rr.send_blueprint(
    rrb.Spatial2DView(
        overrides={"points": [rrb.components.Visible(False)]}
        overrides={
            "hidden_subtree": [rrb.components.Visible(False)],
            "hidden_subtree/child0": [rrb.components.Visible(False)],
            "hidden_subtree/child1": [rrb.components.Visible(False)],
            # …
            "non_interactive_subtree": [rrb.components.Interactive(False)],
            "non_interactive_subtree/child0": [rrb.components.Interactive(False)],
            "non_interactive_subtree/child1": [rrb.components.Interactive(False)],
            # …
        }
    ),
)
```

After:
```python
rr.send_blueprint(
    rrb.Spatial2DView(
        overrides={
            "hidden_subtree": rrb.EntityBehavior(visible=False),
            "hidden_subtree/not_hidden": rrb.EntityBehavior(visible=True),
            "non_interactive_subtree": rrb.EntityBehavior(interactive=False),
        }
    )
)
```

### Visible time range overrides have to specify the underlying archetype

(Note that this functionality broken in at least Rerun 0.21 and 0.22 but is fixed now. See [#8557](https://github.com/rerun-io/rerun/issues/8557))

Before:
```python
# …
overrides={
    "helix/structure/scaffolding/beads": [
        rrb.VisibleTimeRange(
            "stable_time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.3),
            end=rrb.TimeRangeBoundary.cursor_relative(seconds=0.3),
        ),
    ],
},
# …
```

After:
```python
# …
overrides={
    "helix/structure/scaffolding/beads": rrb.VisibleTimeRanges(
            timeline="stable_time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.3),
            end=rrb.TimeRangeBoundary.cursor_relative(seconds=0.3)
        ),
}
# …
```
… or respectively for multiple timelines:
```python
# …
overrides={
    "helix/structure/scaffolding/beads": rrb.VisibleTimeRanges([
        rrb.VisibleTimeRange(
            timeline="stable_time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.3),
            end=rrb.TimeRangeBoundary.cursor_relative(seconds=0.3)
        ),
        rrb.VisibleTimeRange(
            timeline="index",
            start=rrb.TimeRangeBoundary.absolute(seq=10),
            end=rrb.TimeRangeBoundary.absolute(seq=100)
        ),
    ])
}
# …
```

## Types for time series plots are now plural

The `Scalar`/`SeriesPoints`/`SeriesLines` archetypess have been deprecated in favor of
`Scalars`/`SeriesPoints`/`SeriesLines` since you can now have a multiple
scatter plots or lines on the same archetype.


Before:
```python
rr.log("trig/sin", rr.SeriesLines(color=[s0, 255, 0], name="cos(0.01t)", width=4), static=True)

for t in range(int(tau * 2 * 100.0)):
    rr.set_time("step", sequence=t)
    rr.log("trig/sin", rr.Scalar(sin(float(t) / 100.0)))
```

After:
```python
rr.log("trig/sin", rr.SeriesLines(colors=[255, 0, 0], names="sin(0.01t)", widths=2), static=True)

for t in range(int(tau * 2 * 100.0)):
    rr.set_time("step", sequence=t)
    rr.log("trig/sin", rr.Scalars(sin(float(t) / 100.0)))
```
<!-- This is trivial enough across languages why I left it at a python only example -->

The old types still work for the moment but will be removed in a future release.

## Consistent constructor naming of `Asset3D` across C++ and Rust

We've deprecated inconsistent constructors with following replacements:
- 🦀 Rust: `from_file` -> `from_file_path`
- 🌊 C++:
    - `from_file` -> `from_file_path`
    - `from_bytes` -> `from_file_contents`

## Jupyter notebooks

### Explicit `Viewer` imports

We've removed `notebook` from the root `rerun` namespace. `Viewer` must now be imported directly:

Before:
```python
viewer = rr.notebook.Viewer()
viewer.display()
```

After:
```python
from rerun.notebook import Viewer

viewer = Viewer()
viewer.display()
```

`rr.notebook_show` is still available in the root `rerun` namespace.
