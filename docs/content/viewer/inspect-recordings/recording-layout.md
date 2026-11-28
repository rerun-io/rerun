---
title: Recording layout and blueprints
order: 200
description: Understand what controls a recording layout and which layout applies
---

A recording supplies the data; a **recording blueprint** describes how the Viewer displays it.
It controls the layout, view settings, and panel states, independently of the recorded data.
If you can change how something looks in the Viewer, that change is part of the recording blueprint.
A [table blueprint](../browse-datasets/table-layout.md) configures dataset and catalog tables separately.

## What a recording blueprint holds

| Topic | What it covers | In the Viewer | From code |
| --- | --- | --- | --- |
| Panel state | Expanded or collapsed panels | Layout buttons | [Panel state](configure-from-code.md#controlling-panel-state) |
| Arrangement | Containers, tabs, and view placement | [Viewport and blueprint panel](configure-layout.md#viewport) | [Containers](configure-from-code.md#combining-multiple-views) |
| Views | View classes and their settings | Blueprint panel | [Create a view](configure-from-code.md#creating-a-simple-view) |
| View contents | Origin and entity path filter | Select a view | [View contents](configure-from-code.md#specifying-view-contents) |
| View properties | Background, bounds, eye, axes, and visible time ranges | Selection panel | [View properties](configure-from-code.md#advanced-customization) |
| Per-entity display | Visualizers, defaults, overrides, and component mappings | Select an entity in a view | [Customize a visualization](../view-concepts/customize-views.md) |
| Timeline panel | Timeline and playback settings | Timeline panel | [Blueprint API](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) |

## Which recording blueprint applies?

For live streams and directly imported recordings, the Viewer associates recording blueprints with the [application ID](../../log/data-model/recordings.md#application-ids).
Recordings with the same application ID can share a layout.
A catalog dataset can have several registered recording blueprints, including a default used when a segment is opened.
The Viewer catalog can register embedded recording blueprints when it opens an `.rrd`.
See [catalog blueprints](../../catalog/catalog-object-model.md#blueprints).

## Heuristic, default, and current recording blueprints

The **heuristic** recording blueprint is generated from the data when no other layout is available.
A supplied recording blueprint can also generate views automatically when `auto_views` is enabled.
The **default** is supplied from code, a file, or the dataset; when none is set, the heuristic layout is used.
The **current** recording blueprint is the layout currently shown and edited in the Viewer.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/fe1fcf086752f5d7cdd64b195fb3a6cb99c50737_current_default_heuristic.png" width="550px" alt="Current, default, and heuristic blueprints">
  <img class="d2-light" src="https://static.rerun.io/current_default_heuristic_invert/f1f07f49b7e90cfeeb6a0c8bbe47004a9d7593ff/full.png" width="550px" alt="Diagram of how heuristics, user edits and a default blueprint from code or .rrd/.rbl files feed the current blueprint shown in the viewport">
</div>


<span id="why-the-viewer-created-these-views"></span>

When generating views automatically, the Viewer:

-   Creates views for each view class that can show your data, for example one 3D view and one time series view.
-   Spatial views are created per _subspace_: a [`Pinhole`](../../reference/types/archetypes/pinhole.md) starts a new 2D space, so each camera gets its own 2D view.
-   2D views are split by image size, so images of different sizes are not stacked in the same view.
-   At most 8 views of the same class are created automatically.


To control automatic views from code, see [Keep or stop automatic views](configure-from-code.md#keep-or-stop-automatic-views).

## Save and open `.rbl` files

Choose **Save blueprint…** in the File menu to save the current recording blueprint.
Open an `.rbl` with **Open file…** or drag it into the Viewer.
Recording blueprint files can be shared and version controlled.
For loading from code, including C++, see [Loading blueprints from any language](configure-from-code.md#loading-blueprints-from-any-language).

## Select a recording blueprint for a dataset

<img width="507" src="https://static.rerun.io/blueprint_dropdown/9cd8260a49672534c1fb7d15883eae87ede1ee34/full.png" alt="Blueprint dropdown listing the heuristic blueprint and the dataset's blueprints">

For a recording opened from a catalog dataset, there's a dropdown in the top bar to switch between the recording blueprints registered for that dataset.
The default recording blueprint is marked with a star.

The dropdown also has **Heuristic** to switch to the automatic heuristic recording blueprint.
A recording blueprint without a name property is listed as `<unnamed>`.

In the list, the "…" button of the current recording blueprint has **Export .rbl…**, which saves it to a file, and **Revert changes**, which reverts local changes to the recording blueprint.

See [Blueprints](../../catalog/catalog-object-model.md#blueprints) for how to register blueprints on a dataset.


<span id="reset-behavior"></span>

## Reset a recording blueprint

- **Reset current blueprint** discards local edits and restores the state the current recording blueprint started from. A catalog recording blueprint is not downloaded again.
- **Reset to default blueprint** returns to the supplied default, or the heuristic layout if no default is set.
- **Reset to heuristic blueprint** generates a new layout from the data.

The reset button is in the blueprint panel; the other two commands are in the **View** menu and command palette.

## What the Viewer keeps between sessions

The native Viewer remembers recording blueprint edits for an application ID across restarts.
A newly supplied default does not automatically replace those edits; use **Reset to default blueprint** or send the recording blueprint with `make_active=True`.
<span id="why-is-my-blueprint-ignored"></span>

See [Why is my blueprint ignored?](blueprint-ignored.md).

<span id="3-programmatically"></span>

## How a recording blueprint is stored

Under the hood, recording blueprints are data. They are structured using the same [Entity Component System](../../log/data-model/entity-component.md) as your recordings, but with blueprint-specific archetypes and a separate blueprint timeline. This architecture provides several advantages:

-   **Anything you modify in the Viewer can be saved and shared** as a blueprint file
-   **Recording blueprints can be produced programmatically** using just the Rerun SDK without depending on the Viewer; see [Configure a recording layout from code](configure-from-code.md)
-   **Blueprint data uses the same component types as logged data**, so [blueprint overrides](../view-concepts/customize-views.md#set-a-value-for-one-entity) can set any component a visualizer reads
-   **Debugging tools for Rerun data** can inspect blueprint state just like recording data

### Viewer operation

The Viewer is designed to be deterministic. Every frame, the Viewer:
1. Takes the current recording blueprint and the active recording
2. Queries container and view archetypes from the blueprint at the current blueprint timeline revision
3. Uses those view specifications to query the data needed from the recording
4. Renders the results
5. Queues any user interactions as new blueprint events on the blueprint timeline

This means the Viewer output is a deterministic function of the blueprint and the recording, with minimal persisted state between frames.
