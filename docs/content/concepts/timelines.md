---
title: Events and Timelines
order: 3
---

## Timelines

Each piece of logged data is associated with one or more timelines.
By default, each log is added to the `log_time` timeline, with a timestamp assigned by the SDK.

You can use the _set time_ functions (Python reference: [set_time_sequence](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_sequence), [set_time_seconds](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_seconds), [set_time_nanos](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_nanos)) to associate logs with other timestamps on other timelines. For example:

snippet: timelines_example

This will add the logged points to the timelines `log_time`, `frame_idx`, and `sensor_time`.
You can then choose which timeline you want to organize your data along in the expanded timeline view in the bottom of the Rerun Viewer.

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

Similarly, [coordinate systems](spaces-and-transforms.md) or [annotation context](annotation-context.md) are typically static.
