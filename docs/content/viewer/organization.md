---
title: How the Viewer is organized
order: 50
---

The Viewer lets you browse and visualize data from your application, files, and catalogs.
Use it to compare recordings in a dataset, inspect a recording in views, and configure how the data is displayed.
The diagram shows how data reaches the Viewer and the two ways you can explore it.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/4d2eab1ca24c2786b174fecc0aa4072d0bed2d3b_viewer_overview_dark.svg" alt="Diagram of how data reaches a recording: a catalog server holds tables and datasets, and the Viewer catalog holds a dataset named after the application ID. A segment or a linked table row opens as a recording, together with the assets of the dataset. A live stream or an imported file opens as a recording directly. A recording contains views, which draw entities with visualizers.">
  <img class="d2-light" src="https://static.rerun.io/737bb754f81a0c6e2aa58d55bc42b51003ae0d0e_viewer_overview_light.svg" alt="Diagram of how data reaches a recording: a catalog server holds tables and datasets, and the Viewer catalog holds a dataset named after the application ID. A segment or a linked table row opens as a recording, together with the assets of the dataset. A live stream or an imported file opens as a recording directly. A recording contains views, which draw entities with visualizers.">
</div>

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/8e19ae56baa1febb9f1b7faeb6cff0cabe561ebb_viewer_overview_legend_dark.svg" alt="Legend: green marks browsing datasets and tables, with many segments or rows at a time and a table blueprint. Purple marks visually inspecting recordings, with one recording at a time and a recording blueprint.">
  <img class="d2-light" src="https://static.rerun.io/f4cf945481fc6bc369f57ef1339c4077fd22079c_viewer_overview_legend_light.svg" alt="Legend: green marks browsing datasets and tables, with many segments or rows at a time and a table blueprint. Purple marks visually inspecting recordings, with one recording at a time and a recording blueprint.">
</div>

## Two ways to explore data

- **Browse datasets and tables:** Compare many segments or table rows at once, using a table or card layout. A table blueprint controls columns, filters, layouts, and embedded previews. See [Browse datasets and tables](browse-datasets.md) and [Configure table layouts and segment previews](browse-datasets/table-layout.md).
- **Inspect recordings:** Open a segment or linked row, or inspect a stream or imported file directly. A recording blueprint controls the views, containers, settings, and panels used to display one recording. See [Inspect recordings](inspect-recordings.md) and [Recording layout and blueprints](inspect-recordings/recording-layout.md).

A table preview can use the same view classes as a recording, but the table and recording layouts are configured separately.

## From logged data to the Viewer

The Sources panel brings together [live streams](../log/sinks-and-modes/operating-modes.md), [opened files](../ingest/open-any-file.md), catalog datasets, and [tables sent from code](../log/send-data/send-table.md).
As the diagram shows, a live stream or an imported file opens as a recording.
A catalog holds datasets and tables; you can open a dataset segment or a linked table row as a recording, along with the dataset's assets.
The built-in Viewer catalog groups local `.rrd` files by [application ID](../log/data-model/recordings.md#application-ids), so you can browse their segments together without starting a separate server.
See [Browse recordings in a dataset](browse-datasets/open-data.md) to open and compare a collection, or [Browse and annotate tables](browse-datasets/tables.md) for arbitrary tabular data.

A [recording](../log/data-model/recordings.md) contains data organized into [entities and components](../log/data-model/entity-component.md), with changes indexed by [timelines](../log/data-model/timelines.md).
Views display entities, visualizers read their components, and the timeline panel controls which point on a timeline you inspect.
Learn more about [views and visualizers](view-concepts/customize-views.md).
