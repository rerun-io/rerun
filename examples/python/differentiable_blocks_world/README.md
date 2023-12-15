---
title: "Differentiable Blocks World: Qualitative 3D Decomposition by Rendering Primitives"
source: https://github.com/rerun-io/differentiable-blocksworld
tags: [3D, mesh, pinhole-camera]
thumbnail: https://static.rerun.io/dbw/1da9e778d5fc9875a28a1fd74b61654c287e950d/480w.png
thumbnail_dimensions: [480, 311]
---

Finding a textured mesh decomposition from a collection of posed images is a very challenging optimization problem. "Differentiable Block Worlds" by Tom Monnier et al. shows impressive results using differentiable rendering. Here we visualize how this optimization works using the Rerun SDK.

https://vimeo.com/865326948?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7309

In "Differentiable Blocks World: Qualitative 3D Decomposition by Rendering Primitives" the authors describe an optimization of a background icosphere, a ground plane, and multiple superquadrics. The goal is to find the shapes and textures that best explain the observations.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dbw-overview/83fe4a19b65b2c9a5c0e10aef00e4a82026e2b46/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dbw-overview/83fe4a19b65b2c9a5c0e10aef00e4a82026e2b46/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dbw-overview/83fe4a19b65b2c9a5c0e10aef00e4a82026e2b46/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dbw-overview/83fe4a19b65b2c9a5c0e10aef00e4a82026e2b46/1200w.png">
  <img src="https://static.rerun.io/dbw-overview/83fe4a19b65b2c9a5c0e10aef00e4a82026e2b46/full.png" alt="">
</picture>

The optimization is initialized with an initial set of superquadrics ("blocks"), a ground plane, and a sphere for the background. From here, the optimization can only reduce the number of blocks, not add additional ones.

https://vimeo.com/865327350?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6497

A key difference to other differentiable renderers is the addition of transparency handling. Each mesh has an opacity associated with it that is optimized. When the opacity becomes lower than a threshold the mesh is discarded in the visualization. This allows to optimize the number of meshes.

https://vimeo.com/865327387?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7037

To stabilize the optimization and avoid local minima, a 3-stage optimization is employed:
1. the texture resolution is reduced by a factor of 8,
2. the full resolution texture is optimized, and
3. transparency-based optimization is deactivated, only optimizing the opaque meshes from here.

https://vimeo.com/865329177?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:8845

Check out the [project page](https://www.tmonnier.com/DBW/), which also contains examples of physical simulation and scene editing enabled by this kind of scene decomposition.

Also make sure to read the [paper](https://arxiv.org/abs/2307.05473) by Tom Monnier, Jake Austin, Angjoo Kanazawa, Alexei A. Efros, Mathieu Aubry. Interesting study of how to approach such a difficult optimization problem.
