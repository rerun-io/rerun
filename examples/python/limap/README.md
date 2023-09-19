---
title: "3D Line Mapping Revisited"
python: hhttps://github.com/rerun-io/limap
tags: [2D, 3D, structure-from-motion, time-series, line-detection, pinhole-camera]
thumbnail: https://static.rerun.io/1c99ab95ad2a9e673effa0e104f5240912c80850_limap_480w.png
thumbnail_dimensions: [480, 277]
---

Human-made environments contain a lot of straight lines, which are currently not exploited by most mapping approaches. With their recent work "3D Line Mapping Revisited" Shaohui Liu et al. take steps towards changing that.

https://vimeo.com/865327785?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:5819

The work covers all stages of line-based structure-from-motion: line detection, line matching, line triangulation, track building and joint optimization. As shown in the figure, detected points and their interaction with lines is also used to aid the reconstruction.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/924954fe0cf39a4e02ef51fc48dd5a24bd618cbb_limap-overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/1c3528db7299ceaf9b7422b5be89c1aad805af7f_limap-overview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/f6bab491a2fd0ac8215095de65555b66ec932326_limap-overview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/8cd2c725f579dbef19c63a187742e16b6b67cf80_limap-overview_1200w.png">
  <img src="https://static.rerun.io/8d066d407d2ce1117744555b0e7691c54d7715d4_limap-overview_full.png" alt="">
</picture>

LIMAP matches detected 2D lines between images and computes 3D candidates for each match. These are scored, and only the best candidate one is kept (green in video). To remove duplicates and reduce noise candidates are grouped together when they likely belong to the same line.

https://vimeo.com/865905458?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1000:767

Focusing on a single line, LIMAP computes a score for each candidate (the brighter, the higher the cost). These scores are used to decide which line candidates belong to the same line. The final line shown in red is computed based on the candidates that were grouped together.

https://vimeo.com/865973521?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1000:767

Once the lines are found, LIMAP further uses point-line associations to jointly optimize lines and points. Often 3D points lie on lines or intersections thereof. Here we highlight the line-point associations in blue.

https://vimeo.com/865973652?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1000:767

Human-made environments often contain a lot of parallel and orthogonal lines. LIMAP allows to globally optimize the lines by detecting sets that are likely parallel or orthogonal. Here we visualize these parallel lines. Each color is associated with one vanishing point.

https://vimeo.com/865973669?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1000:767

There is a lot more to unpack, so check out the [paper](https://arxiv.org/abs/2303.17504) by Shaohui Liu, Yifan Yu, Rémi Pautrat, Marc Pollefeys, Viktor Larsson. It also gives an educational overview of the strengths and weaknesses of both line-based and point-based structure-from-motion.
