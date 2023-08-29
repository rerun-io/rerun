---
title: "TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement"
python: https://github.com/rerun-io/tapnet
tags: [2D, point-tracking, time-series, tensor, jax]
thumbnail: https://static.rerun.io/991f089320edd15d5b2756f664ec6afcff802bc5_tapir_480w.png
---


Tracking any point in a video is a fundamental problem in computer vision. The paper “TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” by Carl Doersch et al. significantly improved over prior state-of-the-art.

https://www.youtube.com/watch?v=5EixnuJnFdo?playlist=5EixnuJnFdo&loop=1&hd=1&rel=0&autoplay=1

“TAPIR: Tracking Any Point with per-frame Initialization and temporal Refinement” proposes a two-stage approach: (1) compare the query point's feature with the target image features to estimate an initial track; (2) iteratively refine by taking neighboring frames into account.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/f47b954cd1f7a1109df1419b39cc020a364f098d_tapir_overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d792402b789613bfda8354573688e2abf1a7d669_tapir_overview_768w.png">
  <img style="max-width: 30em; width: 100%; margin-left: auto; margin-right: auto;" src="https://static.rerun.io/7f5e5bb87e77aa9ed0cfbf694eaee8ecdb89cafa_tapir_overview_full.png" alt="">
</picture>

In the first stage the image features in the query image at the query point are compared to the feature maps of the other images using the dot product. The resulting similarity map (or “cost volume”) gives a high score for similar image features.

https://www.youtube.com/watch?v=dqvcIlk55AM?playlist=dqvcIlk55AM&loop=1&hd=1&rel=0&autoplay=1

From here, the position of the point is predicted as a heatmap. In addition, the probabilities that the point is occluded and whether its position is accurate are predicted. Only when predicted as non-occluded and accurate a point is classified as visible for a given frame.

https://www.youtube.com/watch?v=T7w8dXEGFzY?playlist=T7w8dXEGFzY&loop=1&hd=1&rel=0&autoplay=1

The previous step gives an initial track but it is still noisy since the inference is done on a per-frame basis. Next, the position, occlusion and accuracy probabilities are iteratively refined using a spatially and temporally local feature volumes.

https://www.youtube.com/watch?v=mVA_svY5wC4?playlist=mVA_svY5wC4&loop=1&hd=1&rel=0&autoplay=1

Check out the [paper](https://arxiv.org/abs/2306.08637) by Carl Doersch, Yi Yang, Mel Vecerik, Dilara Gokay, Ankush Gupta, Yusuf Aytar, Joao Carreira, and Andrew Zisserman. It also includes a nice visual comparison to previous approaches.
