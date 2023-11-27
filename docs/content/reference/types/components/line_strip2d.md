---
title: "LineStrip2D"
---

A line strip in 2D space.

A line strip is a list of points connected by line segments. It can be used to draw
approximations of smooth curves.

The points will be connected in order, like so:
```text
       2------3     5
      /        \   /
0----1          \ /
                 4
```

## Fields

* points: [`Vec2D`](../datatypes/vec2d.md)

## Links
 * 🌊 [C++ API docs for `LineStrip2D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1LineStrip2D.html)
 * 🐍 [Python API docs for `LineStrip2D`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.LineStrip2D)
 * 🦀 [Rust API docs for `LineStrip2D`](https://docs.rs/rerun/latest/rerun/components/struct.LineStrip2D.html)


## Used by

* [`LineStrips2D`](../archetypes/line_strips2d.md)
