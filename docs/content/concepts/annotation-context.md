---
title: Annotation Context
order: 4
---

## Overview

Any visualization that assigns an identifier ("Class ID") to an instance or entity can benefit from using Annotations.
By using an Annotation Context, you can associate labels and colors with a given class and then re-use
that class across entities.

<!-- Example link should point to `latest` but at the time of writing the samples just got renamed -->
This is particularly useful for visualizing the output of classifications algorithms
(as demonstrated by the [Detect and Track Objects](https://github.com/rerun-io/rerun/tree/main/examples/python/detect_and_track_objects) example),
but can be used more generally for any kind of reoccurring categorization within a Rerun recording.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/bb5f5e06931b4924ce3c0243d8285eee558e8f21_classids_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/43c5455dd453e8a3668f0426c3d8961d22a5471e_classids_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/c445af268f7700536bec97bd54134cfe5a48304e_classids_1024w.png">
  <img src="https://static.rerun.io/7f881338f1970161f52a00f1ddd01d4dcccf8a46_classids_full.png" alt="viewer screenshot showing various tracked objects and their class ids">
</picture>



### Keypoints & Keypoint Connections

Rerun allows you to define keypoints *within* a class.
Each keypoint can define its own properties (colors, labels, etc.) that overwrite its parent class.

A typical example usage of keypoints is annotating the joints of a skeleton within a pose detection:
In that case, the entire detected pose/skeleton is assigned a Class ID and each joint within gets a Keypoint ID.

To help you more with this (and similar) use-case(s), you can also define connections between keypoints
as part of your annotation class description:
The viewer will draw the connecting lines for all connected keypoints whenever that class is used.
Just as with labels & colors this allows you to use the same connection information on any instance that class in your scene.

Keypoints are currently only applicable to 2D and 3D points.

<picture>
  <img src="https://static.rerun.io/98b627503df82a6e04c01133dcf6395b040cbd53_keypoints_full.png" alt="keypoint shown on a 3d skeleton">
</picture>



### Logging an Annotation Context

Annotation Context is typically logged as [timeless](timelines.md#timeless-data) data, but can change over time if needed.

The Annotation Context is defined as a list of Class Descriptions that define how classes are styled
(as well as optional keypoint style & connection).

Annotation contexts are logged with:

* Python: [`log_annotation_context`](https://ref.rerun.io/docs/python/latest/common/annotations/#rerun.log_annotation_context)
* Rust: [`AnnotationContext`](https://docs.rs/rerun/latest/rerun/external/re_log_types/component_types/context/struct.AnnotationContext.html)

code-example: annotation-context


## Affected Entities

Each entity that uses a Class ID component (and optionally Keypoint ID components) will look for
the nearest ancestor that in the [entity path hierarchy](entity-path.md#path-hierarchy-functions) that has an Annotation Context defined.


## Segmentation images

Segmentation images are single channel integer images/tensors where each pixel represents a class id.
By default, Rerun will automatically assign colors to each class id, but by defining an Annotation Context,
you can explicitly determine the color of each class.

* Python: [`log_segmentation_image`](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_segmentation_image)
* Rust: Log a [`Tensor`](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html) with [`TensorDataMeaning::ClassId`](https://docs.rs/rerun/latest/rerun/components/enum.TensorDataMeaning.html#variant.ClassId)

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/b1da782a05e2f7c0048f4bddf9ea29fef7c80b4e_segmentation_image_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/f63cb085ee392f38e6431ab7e8c79aecb1b4e6e1_segmentation_image_768w.png">
  <img src="https://static.rerun.io/716eeff1a99f51a6e77fca85c4e7dccf76b77c69_segmentation_image_full.png" alt="screenshot of a segmentation image">
</picture>

