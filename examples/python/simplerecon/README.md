---
title: "SimpleRecon: 3D Reconstruction Without 3D Convolutions"
python: https://github.com/rerun-io/simplerecon
tags: [3D, depth, time-series, pinhole-camera, mesh]
thumbnail: https://static.rerun.io/394d6544341a45882dcad4f2f5fbaabd74b3d1a3_simplerecon_480w.png
---

SimpleRecon is a back-to-basics approach for 3D scene reconstruction from posed monocular images by Niantic Labs. It offers state-of-the-art depth accuracy and competitive 3D scene reconstruction which makes it perfect for resource-constrained environments.

https://www.youtube.com/watch?v=TYR9_Ql0w7k?playlist=TYR9_Ql0w7k&loop=1&hd=1&rel=0&autoplay=1

SimpleRecon's key contributions include using a 2D CNN with a cost volume, incorporating metadata via MLP, and avoiding computational costs of 3D convolutions. The different frustrums in the visualization show each source frame used to compute the cost volume. These source frames have their features extracted and back-projected into the current frames depth plane hypothesis.

https://www.youtube.com/watch?v=g0dzm-k1-K8?playlist=g0dzm-k1-K8&loop=1&hd=1&rel=0&autoplay=1

SimpleRecon only uses camera poses, depths, and surface normals (generated from depth) for supervision allowing for out-of-distribution inference e.g. from an ARKit compatible iPhone.

https://www.youtube.com/watch?v=OYsErbNdQSs?playlist=OYsErbNdQSs&loop=1&hd=1&rel=0&autoplay=1

The method works well for applications such as robotic navigation, autonomous driving, and AR. It takes input images, their intrinsics, and relative camera poses to predict dense depth maps, combining monocular depth estimation and MVS via plane sweep.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/6074c6c7039eccb14796dffda6e158b4d6a09c0e_simplerecon-overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ed7ded09ee1d32c9adae4b8df0b539a57e2286f0_simplerecon-overview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/431dd4d4c6d4245ccf4904a38e24ff143713c97d_simplerecon-overview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/59058fb7a7a4a5e3d63116aeb7197fb3f32fe19a_simplerecon-overview_1200w.png">
  <img src="https://static.rerun.io/1f2400ba4f3b90f967f9503b855364363f776dbb_simplerecon-overview_full.png" alt="">
</picture>

Metadata incorporated in the cost volume improves depth estimation accuracy and 3D reconstruction quality. The lightweight and interpretable 2D CNN architecture benefits from added metadata for each frame, leading to better performance.

If you want to learn more about the method, check out the [paper](https://arxiv.org/abs/2208.14743) by Mohamed Sayed, John Gibson, Jamie Watson, Victor Prisacariu, Michael Firman, and Clément Godard.
