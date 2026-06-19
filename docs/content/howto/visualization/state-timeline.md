---
title: Visualize state changes
order: 700
---

The [StateTimelineView](../../reference/types/views/state_timeline_view.md) shows how entities transition between discrete states over time. Each entity becomes a horizontal lane, and each logged state is rendered as a colored band that runs until the next change. This is a good fit for state machines, mode transitions, sensor health, or any other piece of data that's better described as "what state am I in right now?" than as a numerical value.

> [!WARNING]
> **Experimental.** The State Timeline view, the [`StateChange`](../../reference/types/archetypes/state_change.md) archetype, and the [`StateConfiguration`](../../reference/types/archetypes/state_configuration.md) archetype are all marked unstable and may change in ways that aren't backwards compatible.

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
