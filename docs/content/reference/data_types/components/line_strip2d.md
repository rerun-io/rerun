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
 * 🐍 [Python API docs for `LineStrip2D`](https://ref.rerun.io/docs/python/HEAD/package/rerun/components/line_strip2d/)
 * 🦀 [Rust API docs for `LineStrip2D`](https://docs.rs/rerun/0.9.0-alpha.6/rerun/components/struct.LineStrip2D.html)


## Used by

* [`LineStrips2D`](../archetypes/line_strips2d.md)
