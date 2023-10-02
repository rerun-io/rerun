---
title: "ViewCoordinates"
---

How we interpret the coordinate system of an entity/space.

For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?

The three coordinates are always ordered as [x, y, z].

For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
down, and the Z axis points forward.

The following constants are used to represent the different directions.
 Up = 1
 Down = 2
 Right = 3
 Left = 4
 Forward = 5
 Back = 6



## Used by

* [`Pinhole`](../archetypes/pinhole.md)
* [`ViewCoordinates`](../archetypes/view_coordinates.md)
