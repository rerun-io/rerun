---
title: Events and Timelines
order: 3
---

## Timelines

Each piece of logged data is associated with one or more timelines.
By default, each log is added to the `log_time` timeline, with a timestamp assigned by the SDK.

You can use the _set time_ functions (Python reference: [set_time_sequence](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_sequence), [set_time_seconds](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_seconds), [set_time_nanos](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.set_time_nanos)) to associate logs with other timestamps on other timelines. For example:

code-example: timelines_example

This will add the logged points to the timelines `log_time`, `frame_idx`, and `sensor_time`.
You can then choose which timeline you want to organize your data along in the expanded timeline view in the bottom of the Rerun Viewer.

## Events

An _event_ refer to an instance of logging one or more component batches to one or more timelines. In the viewer, the Time panel provide a graphical representation of these events across time and entities. 


## Timeless data

The [`rr.log()`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log) function has a `timeless=False` default argument.
If `timeless=True` is used instead, the entity become *timeless*. Timeless entities belong to all timelines (existing ones, and ones not yet created) and are shown leftmost in the time panel in the viewer.
This is useful for entities that aren't part of normal data capture, but set the scene for how they are shown.
For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be timeless.

Similarly, [coordinate systems](spaces-and-transforms.md) or [annotation context](annotation-context.md) are typically timeless.
