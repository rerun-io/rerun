---
title: "Differentiable Blocks World: Qualitative 3D Decomposition by Rendering Primitives"
python: https://github.com/rerun-io/differentiable-blocksworld
tags: [3D, mesh, pinhole-camera]
thumbnail: https://static.rerun.io/fd44aa668cdebc6a4c14ff038e28f48cfb83c5ee_dbw_480w.png
---

Finding a textured mesh decomposition from a collection of posed images is a very challenging optimization problem. "Differentiable Block Worlds" by Tom Monnier et al. shows impressive results using differentiable rendering. I visualized how this optimization works using the Rerun SDK.

<div style="padding:64.97% 0 0 0;position:relative;margin-bottom:1.25rem;"><iframe src="https://player.vimeo.com/video/865326948?h=9c5b7559c9&autoplay=1&loop=1&autopause=0&background=1&muted=1&title=0&byline=0&portrait=0" style="position:absolute;top:0;left:0;width:100%;height:100%;" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe></div><script src="https://player.vimeo.com/api/player.js"></script>

In "Differentiable Blocks World: Qualitative 3D Decomposition by Rendering Primitives" the authors describe an optimization of a background icosphere, a ground plane, and multiple superquadrics. The goal is to find the shapes and textures that best explain the observations.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/71b822942cb6ce044d6f5f177350c61f0ab31d80_dbw-overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/9586ea6a3f73d247984f951c07d9cf40dcdf23d2_dbw-overview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/89bab0c74b2bbff84a606cc3a400f208e1aaadeb_dbw-overview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/7c8bec373d0a6c71ea05ffa696acb981137ca579_dbw-overview_1200w.png">
  <img src="https://static.rerun.io/a8fea9769b734b2474a1e743259b3e4e68203c0f_dbw-overview_full.png" alt="">
</picture>

The optimization is initialized with an initial set of superquadrics ("blocks"), a ground plane, and a sphere for the background. From here, the optimization can only reduce the number of blocks, not add additional ones.

<div style="padding:64.97% 0 0 0;position:relative;margin-bottom:1.25rem;"><iframe src="https://player.vimeo.com/video/865327350?h=9c5b7559c9&autoplay=1&loop=1&autopause=0&background=1&muted=1&title=0&byline=0&portrait=0" style="position:absolute;top:0;left:0;width:100%;height:100%;" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe></div><script src="https://player.vimeo.com/api/player.js"></script>

A key difference to other differentiable renderers is the addition of transparency handling. Each mesh has an opacity associated with it that is optimized. When the opacity becomes lower than a threshold the mesh is discarded in the visualization. This allows to optimize the number of meshes.

<div style="padding:64.97% 0 0 0;position:relative;margin-bottom:1.25rem;"><iframe src="https://player.vimeo.com/video/865327387?h=9c5b7559c9&autoplay=1&loop=1&autopause=0&background=1&muted=1&title=0&byline=0&portrait=0" style="position:absolute;top:0;left:0;width:100%;height:100%;" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe></div><script src="https://player.vimeo.com/api/player.js"></script>

To stabilize the optimization and avoid local minima, a 3-stage optimization is employed:
1. the texture resolution is reduced by a factor of 8,
2. the full resolution texture is optimized, and
3. transparency-based optimization is deactivated, only optimizing the opaque meshes from here.

https://vimeo.com/865329177

https://vimeo.com/865329177?autoplay=1&loop=1&autopause=0&background=1&muted=1

<div style="padding:64.97% 0 0 0;position:relative;margin-bottom:1.25rem;color-scheme:unset;"><iframe src="https://player.vimeo.com/video/865329177?h=9c5b7559c9&autoplay=1&loop=1&autopause=0&background=1&muted=1&title=0&byline=0&portrait=0" style="position:absolute;top:0;left:0;width:100%;height:100%;" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe></div><script src="https://player.vimeo.com/api/player.js"></script>

<div style="padding:64.97% 0 0 0;position:relative;margin-bottom:1.25rem;color-scheme:unset;"><iframe src="https://player.vimeo.com/video/865329177?h=9c5b7559c9&autoplay=1&loop=1&autopause=0&title=0&byline=0&portrait=0" style="position:absolute;top:0;left:0;width:100%;height:100%;" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe></div><script src="https://player.vimeo.com/api/player.js"></script>

Check out the [project page](https://www.tmonnier.com/DBW/), which also contains examples of physical simulation and scene editing enabled by this kind of scene decomposition.

Also make sure to read the [paper](https://arxiv.org/abs/2307.05473) by Tom Monnier, Jake Austin, Angjoo Kanazawa, Alexei A. Efros, Mathieu Aubry. Interesting study of how to approach such a difficult optimization problem.
