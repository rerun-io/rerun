---
title: Annotation Context
order: 4
---

# Overview

Any visualization that assigns an identifier ("Class ID") to an instance or entity can profit from using annotations.
By using an Annotation Context, you can associated labels and colors to a given class.

<!-- TODO(andreas) update this example link? -->
This is particularly useful for visualizing the output classifications algorithms
(as demonstrated by the [Detect and Track Objects](https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects) example),
but can be used more generally for any kind of categorization within a scene.

TODO: screenshot

## Keypoints & Keypoint Connections

Rerun allows you to define keypoints within a class.
Each keypoint can define its own properties (colors, labels, etc.) that overwrite its parent class.

A typical example for keypoints would be the joints of a skeleton within a pose detection:
In that case, the entire detected pose/skeleton is assigned a Class ID and each joint within gets a Keypoint ID.

To help you more with this (and similar) usecase(s), you can define connections between keypoints
as part of your annotation class description:
The viewer will draw the connecting lines for all connected keypoints whenever that class is used.
Just as with labels & colors this allows you to use the same connection information on any instance that class in your scene.

Keypoints are currently only applicable to 2D and 3D points.

TODO: screenshot

# How to log an Annotation Context

Annotation Context is typically logged as [timeless](TODO:) data, but can change over time if needed.

The Annotation Context is defined as a list of Class Descriptions that define how classes are styled
(as well as optional keypoint style & connection).

Annotation contexts are logged with:

* Python: [`log_annotation_context`](TODO:)
* Rust: [TODO:]

code-example: annotation-context


# Which Entities are affected 

Each entity that uses a Class ID component (and optionally Keypoint ID components) will look for
the nearest ancestor that in the [entity path hierarchy](TODO:) that has an Annotation Context defined.

# Segmentation images

Segmentation images are single channel integer images/tensors where each pixel represents a class id.
By default, Rerun will automatically assign colors to each class id, but by defining an Annotation Context,
you can explicitly determine the color of each class.

TODO: code links.
