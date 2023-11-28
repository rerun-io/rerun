---
title: "AnnotationContext"
---

The `AnnotationContext` provides additional information on how to display entities.

Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
the labels and colors will be looked up in the appropriate
`AnnotationContext`. We use the *first* annotation context we find in the
path-hierarchy when searching up through the ancestors of a given entity
path.

## Fields

* class_map: [`ClassDescriptionMapElem`](../datatypes/class_description_map_elem.md)

## Links
 * 🌊 [C++ API docs for `AnnotationContext`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1AnnotationContext.html)
 * 🐍 [Python API docs for `AnnotationContext`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.AnnotationContext)
 * 🦀 [Rust API docs for `AnnotationContext`](https://docs.rs/rerun/latest/rerun/components/struct.AnnotationContext.html)


## Used by

* [`AnnotationContext`](../archetypes/annotation_context.md)
