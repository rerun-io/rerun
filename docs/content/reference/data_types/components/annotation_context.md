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
 * 🐍 Python API docs: https://ref.rerun.io/docs/python/HEAD/package/rerun/components/annotation_context/
 * 🦀 Rust API docs: https://docs.rs/rerun/latest/rerun/components/struct.AnnotationContext.html


## Used by

* [`AnnotationContext`](../archetypes/annotation_context.md)
