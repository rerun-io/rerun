---
title: Annotation Context
order: 4
---

## Overview

Any visualization that assigns an identifier ("Class ID") to an instance or entity can profit from using Annotations.
By using an Annotation Context, you can associate labels and colors with a given class and then re-use
that class across entities.

This is particularly useful for visualizing the output classifications algorithms
(as demonstrated by the [Detect and Track Objects](https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects) example),
but can be used more generally for any kind of reoccurring categorization within a Rerun recording.

![classids](https://static.rerun.io/5508e3fd5b2fdc020eda0bd545ccb97d26a01303_classids.png)


### Keypoints & Keypoint Connections

Rerun allows you to define keypoints *within* a class.
Each keypoint can define its own properties (colors, labels, etc.) that overwrite its parent class.

A typical example for keypoints would be the joints of a skeleton within a pose detection:
In that case, the entire detected pose/skeleton is assigned a Class ID and each joint within gets a Keypoint ID.

To help you more with this (and similar) usecase(s), you can define connections between keypoints
as part of your annotation class description:
The viewer will draw the connecting lines for all connected keypoints whenever that class is used.
Just as with labels & colors this allows you to use the same connection information on any instance that class in your scene.

Keypoints are currently only applicable to 2D and 3D points.

![keypoints](https://static.rerun.io/a8be4dff9cf1d2793d5a5f0d5c4bb058d1430ea8_keypoints.png)


### Logging an Annotation Context

Annotation Context is typically logged as [timeless](./timelines#timeless-data) data, but can change over time if needed.

The Annotation Context is defined as a list of Class Descriptions that define how classes are styled
(as well as optional keypoint style & connection).

Annotation contexts are logged with:

* Python: [`log_annotation_context`](https://ref.rerun.io/docs/python/latest/common/annotations/#rerun.log_annotation_context)
* Rust: [`AnnotationContext`](https://docs.rs/rerun/latest/rerun/external/re_log_types/component_types/context/struct.AnnotationContext.html)

code-example: annotation-context


## Affected Entities

Each entity that uses a Class ID component (and optionally Keypoint ID components) will look for
the nearest ancestor that in the [entity path hierarchy](./entity-path#path-hierarchy-functions) that has an Annotation Context defined.


## Segmentation images

Segmentation images are single channel integer images/tensors where each pixel represents a class id.
By default, Rerun will automatically assign colors to each class id, but by defining an Annotation Context,
you can explicitly determine the color of each class.

* Python: [`log_segmentation_image`](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_segmentation_image)
* Rust: Log a [`Tensor`](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html) with [`TensorDataMeaning::ClassId`](https://docs.rs/rerun/latest/rerun/components/enum.TensorDataMeaning.html#variant.ClassId)

![segmentation image](https://static.rerun.io/7c47738b791a7faaad8f0221a78c027300d407fc_segmentation_image.png)
