---
title: Annotation Context
order: 500
description: Share labels and colors across entities by mapping class IDs to styles
---

## Overview

Any visualization that assigns an identifier ("Class ID") to an instance or entity can benefit from using Annotations.
By using an Annotation Context, you can associate labels and colors with a given class and then re-use
that class across entities.

<!-- Example link should point to `latest` but at the time of writing the samples just got renamed -->
This is particularly useful for visualizing the output of classification algorithms
(as demonstrated by the [Detect and Track Objects](https://github.com/rerun-io/rerun/tree/main/examples/python/detect_and_track_objects) example),
but can be used more generally for any kind of reoccurring categorization within a Rerun recording.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/classids/7f881338f1970161f52a00f1ddd01d4dcccf8a46/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/classids/7f881338f1970161f52a00f1ddd01d4dcccf8a46/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/classids/7f881338f1970161f52a00f1ddd01d4dcccf8a46/1024w.png">
  <img src="https://static.rerun.io/classids/7f881338f1970161f52a00f1ddd01d4dcccf8a46/full.png" alt="viewer screenshot showing various tracked objects and their class ids">
</picture>



### Keypoints & keypoint connections

Rerun allows you to define keypoints *within* a class.
Each keypoint can define its own properties (colors, labels, etc.) that overwrite its parent class.

A typical example usage of keypoints is annotating the joints of a skeleton within a pose detection.
In that case, the entire detected pose/skeleton is assigned a Class ID and each joint within gets a Keypoint ID.

You can also define connections between keypoints as part of your annotation class description.
The Viewer will draw the connecting lines for all connected keypoints whenever that class is used.
Just as with labels and colors this allows you to use the same connection information on any instance of that class in your scene.

Keypoints are currently only applicable to 2D and 3D points.

<picture>
  <img src="https://static.rerun.io/keypoints/07b268032ab7cd26812de6b83e018b8ab55ed2f2/full.png" alt="keypoint shown on a 3D skeleton">
</picture>



### Logging an annotation context

Annotation Context is typically logged as [static](../../log/data-model/timelines.md#static-data) data, but can change over time if needed.

The Annotation Context is defined as a list of Class Descriptions that define how classes are styled
(as well as optional keypoint style and connection).

Annotation contexts are logged with:

* C++: 🌊[`rerun::AnnotationContext`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1AnnotationContext.html)
* Python: 🐍[`rr.AnnotationContext`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.AnnotationContext)
* Rust: 🦀[`rerun::AnnotationContext`](https://docs.rs/rerun/latest/rerun/archetypes/struct.AnnotationContext.html#)

snippet: tutorials/annotation_context


## Affected entities

Each entity that uses a Class ID component (and optionally Keypoint ID components) will look for
the nearest ancestor in the [entity path hierarchy](../../log/data-model/entity-path.md#path-hierarchy-functions) that has an Annotation Context defined.

For fields that support annotation context, the Viewer automatically uses recorded values when available, otherwise annotation context when Class IDs or Keypoint IDs are present.
You can also select annotation context explicitly with a [visualizer component mapping](component-mappings.md).
Once annotation context is selected as the source for a field, recorded values for that field are ignored.
The Viewer resolves values from Class IDs and Keypoint IDs, then uses the view default or visualizer fallback when the annotation context does not define a value.


## View guides

See the [2D view](../view-types/2d.md#segmentation-images) for segmentation colors and [image overlays](../view-types/2d.md#overlaying-annotations-on-images).
