---
title: Rect2D
order: 2
---
`Rect2D` represents a rectangle in two-dimensional space. The `rect2d` component is always defined by a 4-element list,
with one of several representations:
* XYWH = `[x, y, w, h]`, with x,y = left,top.
* YXHW = `[y, x, h, w]`, with x,y = left,top.
* XYXY = `[x0, y0, x1, y1]`, with x0,y0 = left,top and x1,y1 = right,bottom
* YXYX = `[y0, x0, y1, x1]`, with x0,y0 = left,top and x1,y1 = right,bottom
* XCYCWH = `[x_center, y_center, width, height]`
* XCYCW2H2 = `[x_center, y_center, width/2, height/2]`


It is compatible with [`AnnotationContext`](../../concepts/annotation-context.md). `class_id` can be used to provide
colors and labels from the annotation context. See examples in the
[`AnnotationContext`](../../concepts/annotation-context.md) documentation.

`draw_order` can be used to control how the `Rect2D` entities are drawn relative to other objects within the scene. Higher values are drawn on top of lower values.

## Components and APIs
Primary component: `rect2d`,

Secondary components: `colorrgba`, `radius`, `label`, `classid`, `draw_order`

Python APIs: [log_rect](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_rect), [log_rects](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_rects)

Rust API: [Rect2D](https://docs.rs/rerun/latest/rerun/components/enum.Rect2D.html)

## Simple Example

code-example: rect2d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rect2d_simple/8c06df0ca7e336f76a9ae933017e00493516d13b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rect2d_simple/8c06df0ca7e336f76a9ae933017e00493516d13b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rect2d_simple/8c06df0ca7e336f76a9ae933017e00493516d13b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rect2d_simple/8c06df0ca7e336f76a9ae933017e00493516d13b/1200w.png">
  <img src="https://static.rerun.io/rect2d_simple/8c06df0ca7e336f76a9ae933017e00493516d13b/full.png" alt="">
</picture>
