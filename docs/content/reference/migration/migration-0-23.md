---
title: Migrating from 0.22 to 0.23
order: 989
---

## Timelines are uniquely identified by name
Previously, you could (confusingly) have two timelines with the same name, as long as they had different types (sequence vs temporal).
This is no longer possible.
Timelines are now uniquely identified by name, and if you use different types on the same timeline, you will get a logged warning, and the _latest_ type will be used to interpret the full set of time data.

## 🐍 Python: replaced `rr.set_time_*` with `rr.set_index`
We're moving towards a more explicit API for setting time, where you need to explicitly specify if a time is either a datetime (e.g. `2025-03-03T14:34:56.123456789`) or a timedelta (e.g. `123s`).

Previously we would infer the user intent at runtime based on the value: if it was large enough, it was interpreted as time since the Unix epoch, otherwise it was interpreted as a timedelta.

To this end, we're deprecated `rr.set_time_seconds`, `rr.set_time_nanos`, as well as `rr.set_time_sequence` and replaced them with `rr.set_index`.
`set_index` takes either a `sequence=`, `timedelta=` or `datetime=` argument.

`timedelta` must be either:
* seconds as `int` or `float`
* [`datetime.timedelta`](https://docs.python.org/3/library/datetime.html#datetime.timedelta)
* [`numpy.timedelta64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.timedelta64)

`datetime` must be either:
* seconds since unix epoch (1970-01-01) as `int` or `float`
* [`datetime.datetime`](https://docs.python.org/3/library/datetime.html#datetime.datetime)
* [`numpy.datetime64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.datetime64)

### Migrating
##### `rr.set_sequence("foo", 42)`
New: `rr.set_index("foo", sequence=42)`

##### `rr.set_time_seconds("foo", duration_seconds)`
When using relative times (durations/timedeltas): `rr.set_index("foo", timedelta=duration_seconds)`
You can also pass in a [`datetime.timedelta`](https://docs.python.org/3/library/datetime.html#datetime.timedelta) or [`numpy.timedelta64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.timedelta64) directly.

##### `rr.set_time_seconds("foo", seconds_since_epoch)`
New: `rr.set_index("foo", datetime=seconds_since_epoch)`
You can also pass in a [`datetime.datetime`](https://docs.python.org/3/library/datetime.html#datetime.datetime) or [`numpy.datetime64`](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.datetime64) directly.

##### `rr.set_time_nanos("foo", duration_nanos)`
Either:
* `rr.set_index("foo", timedelta=1e-9 * duration_nanos)`
* `rr.set_index("foo", timedelta=np.timedelta64(duration_nanos, 'ns'))`

The former is subject to (double-precision) floating point precision loss (but still nanosecond precision for timedeltas below less than 100 days in duration), while the latter is lossless.

##### `rr.set_time_nanos("foo", nanos_since_epoch)`
Either:
* `rr.set_index("foo", datetime=1e-9 * nanos_since_epoch)`
* `rr.set_index("foo", datetime=np.datetime64(nanos_since_epoch, 'ns'))`

The former is subject to (double-precision) floating point precision loss (still microsecond precision for the next century), while the latter is lossless.


## 🐍 Python: replaced `rr.Time*Column` with `rr.IndexColumn`
Similarly to the above new `set_index` API, there is also a new `IndexColumn` class that replaces `TimeSequenceColumn`, `TimeSecondsColumn`, and `TimeNanosColumn`.
The migration is very similar to the above.

### Migration
##### `rr.TimeSequenceColumn("foo", values)`
New: `rr.IndexColumn("foo", sequence=values)`

##### `rr.TimeSecondsColumn("foo", duration_seconds)`
New: `rr.IndexColumn("foo", timedelta=duration_seconds)`

##### `rr.TimeSecondsColumn("foo", seconds_since_epoch)`
New: `rr.IndexColumn("foo", datetime=seconds_since_epoch)`

##### `rr.TimeNanosColumn("foo", duration_nanos)`
Either:
* `rr.IndexColumn("foo", timedelta=1e-9 * duration_nanos)`
* `rr.IndexColumn("foo", timedelta=np.timedelta64(duration_nanos, 'ns'))`

The former is subject to (double-precision) floating point precision loss (but still nanosecond precision for timedeltas below less than 100 days in duration), while the latter is lossless.

##### `rr.TimeNanosColumn("foo", nanos_since_epoch)`
Either:
* `rr.IndexColumn("foo", timedelta=1e-9 * nanos_since_epoch)`
* `rr.IndexColumn("foo", timedelta=np.timedelta64(nanos_since_epoch, 'ns'))`

The former is subject to (double-precision) floating point precision loss (still microsecond precision for the next century), while the latter is lossless.

## 🐍 Python: removed `rr.log_components()`, `rr.connect()`, and `rr.connect_tcp()`

These functions were [deprecated](migration-0-22#python-api-changes) in 0.22 and are no longer available.

Calls to `rr.log_components()` API are now superseded by the new partial update API. See the [documentation](../../concepts/latest-at#partial-updates) and the [migration instructions](migration-0-22.md#partial-updates).

Calls to `rr.connect()` and `rr.connect_tcp()` must be changed to [`rr.connect_grpc()`](https://ref.rerun.io/docs/python/0.22.1/common/initialization_functions/#rerun.connect_grpc?speculative-link).