---
title: "TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement"
python: https://github.com/rerun-io/tapnet
tags: [2D, point-tracking, time-series]
thumbnail: https://static.rerun.io/033edff752f86bcdc9a81f7877e0b4411ff4e6c5_structure_from_motion_480w.png
---


Tracking any point in a video is a fundamental problem in computer vision. The recent paper “TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” by Carl Doersch et al. significantly improved over prior state-of-the-art.

https://www.youtube.com/watch?v=5EixnuJnFdo?playlist=5EixnuJnFdo&loop=1&vq=hd720&rel=0

“TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” proposes a two-stage approach: (1) compare the query point's feature with the target image features to estimate an initial track; (2) iteratively refine by taking neighboring frames into account.

# TODO update pictures
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/033edff752f86bcdc9a81f7877e0b4411ff4e6c5_structure_from_motion_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/29f207025a6c5a63e487f95fc6098a4f1f8d9ca3_structure_from_motion_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/6b7914b63f909f2ac5b23530a7d7363178b331cb_structure_from_motion_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/7047a851275c94c2a7e018bd7230dac96c0cea09_structure_from_motion_1200w.png">
  <img src="https://static.rerun.io/b17f8824291fa1102a4dc2184d13c91f92d2279c_structure_from_motion_full.png" alt="Structure From Motion example screenshot">
</picture>

In the first stage the image features in the query image at the query point are compared to the feature maps of the other images using the dot product. The resulting similarity map (or “cost volume”) gives a high score for similar image features.

https://www.youtube.com/watch?v=dqvcIlk55AM?playlist=dqvcIlk55AM&loop=1&vq=hd720&rel=0

From here, the position of the point is predicted as a heatmap. In addition, the probabilities that the point is occluded and whether its position is accurate are predicted. Only when predicted as non-occluded and accurate a point is classified as visible for a given frame.

https://www.youtube.com/watch?v=T7w8dXEGFzY?playlist=T7w8dXEGFzY&loop=1&vq=hd720&rel=0

The previous step gives an initial track but it is still noisy since the inference is done on a per-frame basis. Next, the position, occlusion and accuracy probabilities are iteratively refined using a spatially and temporally local feature volumes.

https://www.youtube.com/watch?v=mVA_svY5wC4?playlist=mVA_svY5wC4&loop=1&vq=hd720&rel=0

Check out the [paper](https://arxiv.org/abs/2306.08637) by Carl Doersch, Yi Yang, Mel Vecerik, Dilara Gokay, Ankush Gupta, Yusuf Aytar, Joao Carreira, and Andrew Zisserman. It also includes a nice visual comparison to previous approaches.
