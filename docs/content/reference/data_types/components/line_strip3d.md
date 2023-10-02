---
title: "LineStrip3D"
---

A line strip in 3D space.

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

* points: [`Vec3D`](../datatypes/vec3d.md)


## Used by

* [`LineStrips3D`](../archetypes/line_strips3d.md)
