---
title: Blueprints
order: 600
---

## Blueprints and recordings

<!-- source: Rerun Design System/Documentation schematics -->
<img src="https://static.rerun.io/6e2095a0ffa4f093deb59848b7c294581ded4678_blueprints_and_recordings.png" width="550px">

When you are working with the Rerun viewer, there are two separate pieces that
combine to produce what you see: the "recording" and the "blueprint."

-   The recording provides the actual data you are visualizing.
-   The blueprint is the configuration that determines how the data from the
    recording is displayed.

Both of these pieces are crucial—without the recording there is nothing to
show, and without the blueprint there is no way to show it. Even if you have
used Rerun before without explicitly loading a blueprint, the Viewer was
actually creating one for you. Without a blueprint, there is literally nothing
for the Viewer to display.

## Loose coupling

The blueprint and the recording are only loosely coupled. Rerun uses the
[application ID](apps-and-recordings.md) to determine whether a blueprint and a
recording should be used together, but they are not directly linked beyond that.

This means that either can be changed independently of the other. Keeping the
blueprint constant while changing the recording will allow you to compare
different datasets using a consistent set of views. On the other hand, changing
the blueprint while keeping a recording constant will allow you to view the same
data in different ways.

## What the blueprint controls

Every aspect of what the Viewer displays is controlled by the blueprint. This
includes the type and content of the different views, the organization and
layout of the different containers, and the configuration and styling properties
of the individual data visualizers (see [Visualizers and Overrides](visualizers-and-overrides.md)
for more details).

In general, if you can modify an aspect of how something looks through the
viewer, you are actually modifying the blueprint. (Note that while there may be
some exceptions to this rule at the moment, the intent is to eventually migrate
all state to the blueprint.)

## Current, default, and heuristics blueprints

<!-- source: Rerun Design System/Documentation schematics -->
<img src="https://static.rerun.io/fe1fcf086752f5d7cdd64b195fb3a6cb99c50737_current_default_heuristic.png" width="550px">

Blueprints may originate from multiple sources.

- The "current blueprint" for a given application ID is the one that is used by the Viewer to display data at any given time. It is updated for each change made to the visualization within the viewer, and may be saved to a blueprint file at any time.
- The "default blueprint" is a snapshot that is set or updated when a blueprint is received from code or loaded from a file. The current blueprint may be reset to default blueprint at any time by using the "reset" button in the blueprint panel's header.
- The "heuristic blueprint" is an automatically-produced blueprint based on the recording data. When no default blueprint is available, the heuristic blueprint is used when resetting the current blueprint. It is also possible to reset to the heuristic blueprint in the selection panel after selecting an application.

## What is a blueprint

Under the hood, the blueprint is just data. It is represented by a
[time-series ECS](./entity-component.md), just like a recording. The only
difference is that it uses a specific set of blueprint archetypes and a special
blueprint timeline. Note that even though the blueprint may be sent over the
same connection, blueprint data is kept in an isolated store and is not mixed
with your recording data.

Although the Rerun APIs for working with blueprint may look different from the
regular logging APIs, they are really just syntactic sugar for logging a
collection of blueprint-specific archetypes to a separate blueprint stream.

Furthermore, when you make any change to the Viewer in the UI, what is actually
happening is the Viewer is creating a new blueprint event and adding it to the
end of the blueprint timeline in the blueprint store.

## Viewer operation

Outside of caching that exists primarily for performance reasons, the viewer
persists very little state frame-to-frame. The goal is for the output of the
Viewer to be a deterministic function of the blueprint and the recording.

Every frame, the Viewer starts with a minimal context of an "active" blueprint,
and an "active" recording. The Viewer then uses the current revision on the
blueprint timeline to query the container and space-view archetypes from the
blueprint store. The space-view archetypes, in turn, specify the paths types
that need to be queried from the recording store in order to render the views.

Any user interactions that modify the blueprint are queued and written back to
the blueprint using the next revision on the blueprint timeline.

## Blueprint architecture motivation

Although this architecture adds some complexity and indirection, the fact that
the Viewer stores all of its meaningful frame-to-frame state in a structured
blueprint data-store has several advantages:

-   Anything you modify in the Viewer can be saved and shared as a blueprint.
-   A blueprint can be produced programmatically using just the Rerun SDK without
    a dependency on the Viewer libraries.
-   The blueprint is capable of representing any data that a recording can
    represent. This means that blueprint-sourced data
    [overrides](visualizers-and-overrides.md#Per-entity-component-override) are
    just as expressive as any logged data.
-   The blueprint is actually stored as a full time-series, simplifying future
    implementations of things like snapshots and undo/redo mechanisms.
-   Debugging tools for inspecting generic Rerun data can be used to inspect
    internal blueprint state.
