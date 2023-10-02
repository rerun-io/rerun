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


## Used by

* [`LineStrips2D`](../archetypes/line_strips2d.md)
