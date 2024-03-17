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

## Links
 * 🌊 [C++ API docs for `AnnotationContext`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1AnnotationContext.html)
 * 🐍 [Python API docs for `AnnotationContext`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.AnnotationContext)
 * 🦀 [Rust API docs for `AnnotationContext`](https://docs.rs/rerun/latest/rerun/archetypes/struct.AnnotationContext.html)

## Examples

### Rectangles

snippet: annotation_context_rects

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png">
  <img src="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/full.png" width="640">
</picture>
</center>

### Segmentation

snippet: annotation_context_segmentation

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1200w.png">
  <img src="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png" width="640">
</picture>
</center>

### Connections

snippet: annotation_context_connections

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1200w.png">
  <img src="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/full.png" width="640">
</picture>
</center>

