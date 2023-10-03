---
title: "AnnotationContext"
---

The `AnnotationContext` provides additional information on how to display entities.

Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
the labels and colors will be looked up in the appropriate
`AnnotationContext`. We use the *first* annotation context we find in the
path-hierarchy when searching up through the ancestors of a given entity
path.

## Components

**Required**: [`AnnotationContext`](../components/annotation_context.md)

## Example

### Segmentation

code-example: annotation_context_segmentation

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1200w.png">
  <img src="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png" width="640">
</picture>
</center>

