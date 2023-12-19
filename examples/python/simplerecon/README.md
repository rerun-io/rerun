<!--[metadata]
title = "SimpleRecon: 3D Reconstruction Without 3D Convolutions"
source = "https://github.com/rerun-io/simplerecon"
tags = ["3D", "depth", "time-series", "pinhole-camera", "mesh"]
thumbnail = "https://static.rerun.io/simplerecon/e309760134e44ba5ca1a547cb310d47a19257e5b/480w.png"
thumbnail_dimensions = [480, 271]
-->


SimpleRecon is a back-to-basics approach for 3D scene reconstruction from posed monocular images by Niantic Labs. It offers state-of-the-art depth accuracy and competitive 3D scene reconstruction which makes it perfect for resource-constrained environments.


https://vimeo.com/865974318?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7627

SimpleRecon's key contributions include using a 2D CNN with a cost volume, incorporating metadata via MLP, and avoiding computational costs of 3D convolutions. The different frustums in the visualization show each source frame used to compute the cost volume. These source frames have their features extracted and back-projected into the current frames depth plane hypothesis.


https://vimeo.com/865974327?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6522

SimpleRecon only uses camera poses, depths, and surface normals (generated from depth) for supervision allowing for out-of-distribution inference e.g. from an ARKit compatible iPhone.


https://vimeo.com/865974337?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:11111

The method works well for applications such as robotic navigation, autonomous driving, and AR. It takes input images, their intrinsics, and relative camera poses to predict dense depth maps, combining monocular depth estimation and MVS via plane sweep.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/simplerecon-overview/84359b6ec787147dc915d0a3fe764537d8212835/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/simplerecon-overview/84359b6ec787147dc915d0a3fe764537d8212835/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/simplerecon-overview/84359b6ec787147dc915d0a3fe764537d8212835/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/simplerecon-overview/84359b6ec787147dc915d0a3fe764537d8212835/1200w.png">
  <img src="https://static.rerun.io/simplerecon-overview/84359b6ec787147dc915d0a3fe764537d8212835/full.png" alt="">
</picture>

Metadata incorporated in the cost volume improves depth estimation accuracy and 3D reconstruction quality. The lightweight and interpretable 2D CNN architecture benefits from added metadata for each frame, leading to better performance.

If you want to learn more about the method, check out the [paper](https://arxiv.org/abs/2208.14743) by Mohamed Sayed, John Gibson, Jamie Watson, Victor Prisacariu, Michael Firman, and Clément Godard.
