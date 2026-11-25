---
title: View types
order: 225
description: Choose a view for your data
index: true
---

<!-- TODO(RR-6530): Connect the view guides with the generated view reference. -->

A view is a panel in the Viewer that displays part of your recording.
Choose a view class to match the data you want to inspect, and combine views to compare different kinds of data.
Use [recording blueprints](inspect-recordings/recording-layout.md) to control their contents and layout.

A view class determines which entities a view can display, how it displays them, and how you can interact with them.
Choose a class when you create a view in the Viewer or from code.

| Your data | View |
| --- | --- |
| Images, video, segmentation masks, bounding boxes, keypoints, or XY plots | [2D](view-types/2d.md) |
| Point clouds, meshes, 3D boxes, camera poses, or depth images | [3D](view-types/3d.md) |
| Sensor readings, measurements, or other numeric signals over time | [Time series](view-types/time-series.md) |
| Robot modes, phases, or other states changing over time | [State timeline](view-types/state-timeline.md) |
| Geographic locations, GPS tracks, or paths | [Map](view-types/map.md) |
| Class scores, histogram bins, or frequency spectra | [Bar chart](view-types/bar-chart.md) |
| Exact values across entities and timestamps in a table | [Dataframe](view-types/dataframe.md) |
| Fields and values in a single item of recorded data, such as an MCAP message | [Raw data](view-types/raw-data.md) |
| Plain text or Markdown documents | [Text document](view-types/text-document.md) |
| Timestamped log messages | [Text log](view-types/text-log.md) |
| Nodes and edges in a graph | [Graph](view-types/graph.md) |
| Multidimensional arrays, such as neural network feature maps or activations | [Tensor](view-types/tensor.md) |
| Audio recordings and their waveforms | [Audio](view-types/audio.md) |

For all view classes and their properties, see the [view reference](../reference/types/views.md).
