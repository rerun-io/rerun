---
title: annotation_context
order: 100
---

The `AnnotationContext` provides additional information on how to display entities.

Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
the labels and colors will be looked up in the appropriate
`AnnotationContext`. We use the *first* annotation context we find in the
path-hierarchy when searching up through the ancestors of a given entity
path.

## Components and APIs

Required:
* annotation_context

## Examples

### Rectangles

code-example: annotation_context_rects


### Segmentation

code-example: annotation_context_segmentation


### Connections

code-example: annotation_context_connections


