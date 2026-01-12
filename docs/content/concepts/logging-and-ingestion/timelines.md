---
title: Events and Timelines
order: 500
---

## Timelines

Each piece of logged data is associated with one or more timelines.

The logging SDK always creates two timelines for you:
* `log_tick` - a sequence timeline with the sequence number of the log call
* `log_time` - a temporal timeline with the time of the log call

You can use the `set_time` function (Python reference: [set_time](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time)) to associate logs with other timestamps on other timelines. For example:

snippet: tutorials/timelines_example

This will add the logged points to the timelines `frame_idx` and `sensor_time`, as well as the automatic timelines `log_tick` and `log_time`.
You can then choose which timeline you want to organize your data along in the expanded timeline view in the bottom of the Rerun Viewer.

### How to log precise times
Rerun supports three types of indices, all encoded as `i64`:
* Sequential
* Timestamp (nanoseconds since Unix epoch)
* Timedelta/duration (nanoseconds)

Here's how you use them:

snippet: concepts/indices

### Reset active timeline & differing data per timeline

You can clear the active timeline(s) at any point using `reset_time`.
This can be particularly useful when you want to log different data for individual timelines as illustrated here:

snippet: concepts/different_data_per_timeline

On one timeline the points will appear blue, on the other they appear red.

### Sending many time points at once
To get full control over the logged timelines you can use [`send_columns`](../../howto/send_columns.md).
This is often a lot more efficient when you already have a chunk of temporal data, e.g. some sensor value over time.


## Events

An _event_ refer to an instance of logging one or more component batches to one or more timelines. In the viewer, the Time panel provide a graphical representation of these events across time and entities.

<picture>
  <img src="https://static.rerun.io/event/57255c0552d76ca2837c2e9581a4dc3534b105a5/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/event/57255c0552d76ca2837c2e9581a4dc3534b105a5/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/event/57255c0552d76ca2837c2e9581a4dc3534b105a5/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/event/57255c0552d76ca2837c2e9581a4dc3534b105a5/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/event/57255c0552d76ca2837c2e9581a4dc3534b105a5/1200w.png">
</picture>


## Static data

The [`rr.log()`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log) function has a `static=False` default argument.
If `static=True` is used instead, the data logged becomes *static*. Static data belongs to all timelines (existing ones, and ones not yet created) and shadows any temporal data of the same type on the same entity.

This is useful for data that isn't part of normal data capture, but sets the scene for how it should be shown.
For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be static.

Similarly, [coordinate systems](transforms.md) or [annotation context](../visualization/annotation-context.md) are typically static.

You can read more about static data in the [dedicated section](static.md).
