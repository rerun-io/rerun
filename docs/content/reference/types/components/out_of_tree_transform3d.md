---
title: "OutOfTreeTransform3D"
---

An out-of-tree affine transform between two 3D spaces, represented in a given direction.

"Out-of-tree" means that the transform only affects its own entity: children don't inherit from it.

## Fields

* repr: [`Transform3D`](../datatypes/transform3d.md)

## Links
 * 🐍 [Python API docs for `OutOfTreeTransform3D`](https://ref.rerun.io/docs/python/nightly/common/components#rerun.components.OutOfTreeTransform3D)
 * 🦀 [Rust API docs for `OutOfTreeTransform3D`](https://docs.rs/rerun/0.9.0-alpha.10/rerun/components/struct.OutOfTreeTransform3D.html)


## Used by

* [`Asset3D`](../archetypes/asset3d.md)
