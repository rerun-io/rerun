---
title: Visualize state changes
order: 700
---

The [StateTimelineView](../../reference/types/views/state_timeline_view.md) shows how entities transition between discrete states over time. Each entity becomes a horizontal lane, and each logged state is rendered as a colored band that runs until the next change. This is a good fit for state machines, mode transitions, sensor health, or any other piece of data that's better described as "what state am I in right now?" than as a numerical value.

## Logging state changes

Use [`StateChange`](../../reference/types/archetypes/state_change.md) to log a transition. Each call marks the start of a new state at the current time; the previous state implicitly ends. The state value is a string, so you can use any label that's meaningful for your application.

snippet: howto/state_timeline[log_changes]

<picture>
  <img src="https://static.rerun.io/state_timeline/90a670f810b9be2fc001a2d83e223215497583d8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/state_timeline/90a670f810b9be2fc001a2d83e223215497583d8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/state_timeline/90a670f810b9be2fc001a2d83e223215497583d8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/state_timeline/90a670f810b9be2fc001a2d83e223215497583d8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/state_timeline/90a670f810b9be2fc001a2d83e223215497583d8/1200w.png">
</picture>

### Notes
- The view groups state changes by entity path, so logging to `/door` and `/window` produces two separate lanes.
- Logging the same state value twice in a row is a no-op for visualization, only transitions to a different value start a new phase.
- Each phase runs from its `StateChange` time to the next `StateChange` time on the same entity. The final phase extends indefinitely.

## Customizing labels, colors, and visibility

To override the default styling, log a [`StateConfiguration`](../../reference/types/archetypes/state_configuration.md) to the same entity. `values`, `labels`, `colors`, and `visible` are parallel arrays — index `i` of each describes the same state value. Anything you don't provide falls back to the default (raw value as label, hashed color, visible).

It is usually best to log `StateConfiguration` as static, since it describes how to display values rather than a moment in time.

snippet: howto/state_timeline[state_config]

<picture>
  <img src="https://static.rerun.io/state_timeline/8b56b1d7ec02bb6d45f6115dcba7c4ba8875942b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/state_timeline/8b56b1d7ec02bb6d45f6115dcba7c4ba8875942b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/state_timeline/8b56b1d7ec02bb6d45f6115dcba7c4ba8875942b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/state_timeline/8b56b1d7ec02bb6d45f6115dcba7c4ba8875942b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/state_timeline/8b56b1d7ec02bb6d45f6115dcba7c4ba8875942b/1200w.png">
</picture>

## Visualize any component as state

You don't have to log [`StateChange`](../../reference/types/archetypes/state_change.md) to use this view. Any component whose data is string-, boolean-, or number-like can drive a lane by **remapping** the visualizer's `StateChange:state` input to read from it instead. This lets you separate how you _model_ your data from how you _visualize_ it. For example, visualizing a robot mode that you logged as a plain string via `AnyValues` or `DynamicArchetype` (the same idea as [Plot any scalar](plot-any-scalar.md), applied to the state slot).

The supported source data types are:

- `Utf8` and `LargeUtf8` (rendered as string states)
- `Boolean` (rendered as two states)
- `Int8`, `Int16`, `Int32`, `Int64`, `UInt8`, `UInt16`, `UInt32`, `UInt64`, `Float16`, `Float32`, and `Float64` (rendered as numeric states)

For background on how visualizers resolve their inputs, see [Component mappings](component-mappings.md) and [Customize views](../../concepts/visualization/customize-views.md).

For example, log a robot mode as a plain string component:

snippet: howto/state_remapping[custom_data]

Then point the state-timeline visualizer at it by remapping `StateChange:state`:

snippet: howto/state_remapping[blueprint]

### Add data by dragging components

You can set up the same mapping interactively: drag a component from the streams tree onto a State Timeline view. If the component is a compatible source (string, boolean, or numeric), a new lane is added that remaps `StateChange:state` from it. Incompatible components (e.g. a blob or tensor) are rejected, as is dropping a component that the view already visualizes.

## Setting up the view via blueprint

The State Timeline view is also created automatically when `StateChange` data is present, but you can also configure it explicitly via the blueprint API:

snippet: howto/state_timeline[blueprint]

<picture>
  <img src="https://static.rerun.io/state_timeline/40a6d7fc78ebebf316160ad33da1971a3b23e857/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/state_timeline/40a6d7fc78ebebf316160ad33da1971a3b23e857/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/state_timeline/40a6d7fc78ebebf316160ad33da1971a3b23e857/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/state_timeline/40a6d7fc78ebebf316160ad33da1971a3b23e857/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/state_timeline/40a6d7fc78ebebf316160ad33da1971a3b23e857/1200w.png">
</picture>
