---
title: "ClassDescription"
---

The description of a semantic Class.

If an entity is annotated with a corresponding `ClassId`, rerun will use
the attached `AnnotationInfo` to derive labels and colors.

Keypoints within an annotation class can similarly be annotated with a
`KeypointId` in which case we should defer to the label and color for the
`AnnotationInfo` specifically associated with the Keypoint.

Keypoints within the class can also be decorated with skeletal edges.
Keypoint-connections are pairs of `KeypointId`s. If an edge is
defined, and both keypoints exist within the instance of the class, then the
keypoints should be connected with an edge. The edge should be labeled and
colored as described by the class's `AnnotationInfo`.

## Fields

* info: [`AnnotationInfo`](../datatypes/annotation_info.md)
* keypoint_annotations: [`AnnotationInfo`](../datatypes/annotation_info.md)
* keypoint_connections: [`KeypointPair`](../datatypes/keypoint_pair.md)

## Links
 * 🌊 [C++ API docs for `ClassDescription`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1datatypes_1_1ClassDescription.html?speculative-link)
 * 🐍 [Python API docs for `ClassDescription`](https://ref.rerun.io/docs/python/stable/common/datatypes#rerun.datatypes.ClassDescription)
 * 🦀 [Rust API docs for `ClassDescription`](https://docs.rs/rerun/latest/rerun/datatypes/struct.ClassDescription.html)


## Used by

* [`ClassDescriptionMapElem`](../datatypes/class_description_map_elem.md)
