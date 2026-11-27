---
title: Browse recordings in a dataset
order: 100
description: Open a collection of recordings and compare its segments
keywords: episode, episodes, recordings, teleoperation, policy evaluation
---

Robotics workflows often produce many recordings: teleoperation episodes, for example, or runs from evaluating a policy.
A dataset gives you an overview of these recordings so you can find, compare, and inspect the episodes you need.
Each recording in a dataset appears as a **segment**.

## Open a collection

For local `.rrd` files, open several files or a folder in the Viewer using the file dialog, drag and drop, or the command line.
The built-in Viewer catalog groups files with the same [application ID](../../log/data-model/recordings.md#application-ids) into a dataset.
Select it under **Viewer catalog** in the Sources panel.
See [Importing files](../../ingest/open-any-file.md) for the available options and formats.

For recordings on a catalog server, connect the Viewer to the server and select a dataset in Sources.
The dataset contains recordings registered as segments; see [Connect to a catalog](../../catalog/get-data-out.md#viewing-datasets) and [register recordings](../../catalog/catalog-object-model.md#datasets).

## Find and compare segments

Open the dataset's **Segments** tab to browse its recordings.
Each row represents a segment, and recording-level properties appear as columns.
Choose which columns to show in the Columns menu, sort by a column, or use the filter bar to narrow the rows.
Switch between table and cards to compare segments in the layout that suits your data, then open a segment to inspect its recording.

See [Properties and segment tables](../../catalog/properties-and-segments.md) for how properties become columns, and [Configure table layouts and segment previews](table-layout.md) for configuring cards and previews.

## Explore the schema

The **Schema** tab shows what data is available across the dataset without opening each recording.
Its schema combines the entities and components found across the dataset's segments and their layers.
Use it to find, for example, the camera, joint, or sensor data you can inspect or query.

Search or browse the entity tree, then select an entity to see its components.
You can also show the archetype and data type for each component as additional columns.
The schema describes the data available across the dataset; an individual segment may contain only some of it.
See [Dataset schema](../../catalog/catalog-object-model.md#schema) for how the schema is built.

<picture>
  <img src="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/full.png" alt="Dataset schema explorer showing an entity tree and the components of the selected entity">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/480w.png">
</picture>

## Share static data with assets

An asset is static data registered on a dataset, such as a robot model, room mesh, or camera calibration.
Register it once instead of copying it into every recording: when you open a segment, the Viewer loads the assets that apply to it alongside the recording.
The Viewer caches downloaded asset data for the connection, so other segments can reuse it without downloading it again.
This is especially useful for preview cards, which can show multiple segments with the same robot model or other shared data at once.

Open the dataset's **Assets** tab to see its assets and the segments they apply to.
Choose **Register asset** and provide the URI of an `.rrd` file that the server can read, or choose a local file to upload if the server supports uploads.
The `.rrd` must contain static data.
For local recordings, you can also open an asset from the command line with `rerun recording.rrd --asset robot.rrd`.

When you register an asset, choose whether it applies to **all segments except these** or **only these segments**.
For example, use the first option for a robot model shared by almost every episode, and the second for a calibration used by specific runs.
You can change the selection later from an asset's menu using **Edit coverage**.

<video width="100%" autoplay loop muted controls>
  <source src="https://static.rerun.io/ffb69f38b2c182b51bb250ac8d17ee406cec920d_asset_coverage_ui.mp4" type="video/mp4" />
</video>

The Python SDK supports the same choices with `DatasetEntry.register_asset()`.
See [Assets](../../catalog/catalog-object-model.md#assets) for registration, storage, and limits.

## Collect recordings while streaming

A live stream appears under **Live**, grouped by application ID, and can be inspected as a recording.
Streaming alone does not create a dataset page for comparing recordings.

To inspect data live and keep it for later analysis, stream to the Viewer and save to an `.rrd` file at the same time with multiple sinks.
You can then open the saved files in the Viewer catalog or register them in a catalog dataset for querying and comparison.
See [Multiple sinks](../../log/sinks-and-modes/sinks.md#multiple-sinks-tee-pattern) for an example.
