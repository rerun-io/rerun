---
title: Migrating from 0.24 to 0.25
order: 985
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Removed deprecated `--serve` CLI argument

Use `--web-viewer` instead.

## Removed the `--drop-at-latency` CLI argument

This feature has been defunct for a while. A better replacement can be tracked [in this issue](https://github.com/rerun-io/rerun/issues/11024).

## Removed the `-o` CLI argument shorthand for `--stdout` in `script_add_args`

Use `--stdout` directly instead.

## Flush takes an optional timeout, and returns errors

When flushing a recording stream you can now give it a maximum time for how long it should block.
The flush will block until either it completes, fails (e.g. because of connection loss), or the timeout is reached.

Previously this could only be configured for gRPC sinks, and it was configured once upon setting up the connection.

In the C++ and Python APIs, negative timeouts used to have special meaning. Now they are no longer permitted.

The Python flush calls now raises an error if the flushing did not complete successfully.

The timeout behavior is also improved: it will only block as long as there is _hope of progress_. If the gRPC connection is severed, the flush will aborted with an error. This means it should be very rare that you need to configure a flush timeout, as it will only block for a long time if there is a very slow connection.

Removed:
 * Python: `flush_timeout_sec` argument of `connect_grpc`
 * Rust: `flush_timeout` argument of `connect_grpc_opts`
 * C++: `rerun::GrpcSink::flush_timeout_sec`


## ❗ Deprecations

### Python 3.9

Support for Python 3.9 is being deprecated. Python 3.9 is past end-of-life. See: https://devguide.python.org/versions/
In the next release, we will fully drop support and switch to Python 3.10 as the minimum supported version.

See an overview for supported python versions [here](https://ref.rerun.io/docs/python/main/common#supported-python-versions).

### `archetype` specification in `AnyValues`

Previously, logging two `AnyValues` with the same field name but different archetype name under the same entity would lead to an inconsistency where the viewer would disambiguate them, but not the dataframe API.

```python
arbitrary_int = 10
example = AnyValues()
example.with_field(
    ComponentDescriptor("component_name", "archetype_name"), arbitrary_int
)
example.with_field(
    ComponentDescriptor("component_name", "different_archetype"), arbitrary_int+1
)
rr.log("/path", example)
```

In the viewer we would see two `component_name` entries under different archetypes but they would not be uniquely queryable.

```python
from rerun.dataframe import load_recording
rec = load_recording("<path_to_logs_above>.rrd")
rec.view(index="log_time", contents="/path").select().schema
# Only shows one `component_name` component
```

To address that, we split this functionality into two utilities:

-   `AnyValues`, which has no archetype name
-   `DynamicArchetype`, which requires an archetype name

When using `DynamicArchetype`, the dataframe API will include the archetype the column names (similar to how built-in components are handled), which reduces the possibility for ambiguity.
In the next release we will remove the ability to specify an `archetype` when creating `AnyValues` to finalize the transition.

```python
arbitrary_int = 10
example = DynamicArchetype("archetype_name")
example.with_component_from_data(
    "component_name", arbitrary_int
)
another_example = DynamicArchetype("another_archetype")
another_example.with_field(
    "component_name", arbitrary_int+1
)
rr.log("/path", example)
rr.log("/path", another_example)
```

```python
from rerun.dataframe import load_recording
rec = load_recording("<path_to_logs_above>.rrd")
rec.view(index="log_time", contents="/path").select().schema
# Only shows two `component_name` components deduplicated by archetype!
```
