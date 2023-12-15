<--[metadata]
title = "TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement"
source = "https://github.com/rerun-io/tapnet"
tags = ["2D", "point-tracking", "time-series", "tensor", "jax"]
thumbnail = "https://static.rerun.io/tapir/f6a7697848c2ac1e7f0b8db5964f39133c520896/480w.png"
thumbnail_dimensions = [480, 288]
-->


Tracking any point in a video is a fundamental problem in computer vision. The paper “TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” by Carl Doersch et al. significantly improved over prior state-of-the-art.

https://vimeo.com/865975034?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:9015

“TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” proposes a two-stage approach:
1. compare the query point's feature with the target image features to estimate an initial track, and
2. iteratively refine by taking neighboring frames into account.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/tapir_overview/9018c62ec8334458936542434b4730ade258b21e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/tapir_overview/9018c62ec8334458936542434b4730ade258b21e/768w.png">
  <img src="https://static.rerun.io/tapir_overview/9018c62ec8334458936542434b4730ade258b21e/full.png" alt="">
</picture>

In the first stage the image features in the query image at the query point are compared to the feature maps of the other images using the dot product. The resulting similarity map (or “cost volume”) gives a high score for similar image features.

https://vimeo.com/865975051?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1:1

From here, the position of the point is predicted as a heatmap. In addition, the probabilities that the point is occluded and whether its position is accurate are predicted. Only when predicted as non-occluded and accurate a point is classified as visible for a given frame.

https://vimeo.com/865975071?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:5052

The previous step gives an initial track but it is still noisy since the inference is done on a per-frame basis. Next, the position, occlusion and accuracy probabilities are iteratively refined using a spatially and temporally local feature volumes.

https://vimeo.com/865975078?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6699

Check out the [paper](https://arxiv.org/abs/2306.08637) by Carl Doersch, Yi Yang, Mel Vecerik, Dilara Gokay, Ankush Gupta, Yusuf Aytar, Joao Carreira, and Andrew Zisserman. It also includes a nice visual comparison to previous approaches.