---
title: Assets without normals are shaded
hidden: true
type: feature
---

### Assets without normals are shaded

OBJ, PLY, glTF and DAE [`Asset3D`](../reference/types/archetypes/asset3d.md) assets that have no vertex normals are now lit and shaded: the Viewer computes flat per-triangle normals for them instead of rendering them unlit.
To keep an asset unlit, include all-zero vertex normals in the file.

Before:

<picture>
  <img src="https://static.rerun.io/mesh-smooth-normals-before/14bf42599bcd760342153578d24df2855c7b9771/full.png" alt="The Rerun logo mesh without normals, rendered as a flat, unlit white silhouette">
</picture>

After:

<picture>
  <img src="https://static.rerun.io/mesh-flat-normals-after/36e12e0553ebd4dc6e1f75541df5cdc950312350/full.png" alt="The same mesh shaded with computed flat normals, with its letters clearly visible">
</picture>
