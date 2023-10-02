---
title: "OutOfTreeTransform3D"
---

An out-of-tree affine transform between two 3D spaces, represented in a given direction.

"Out-of-tree" means that the transform only affects its own entity: children don't inherit from it.

## Fields

* repr: [`Transform3D`](../datatypes/transform3d.md)


## Used by

* [`Asset3D`](../archetypes/asset3d.md)
