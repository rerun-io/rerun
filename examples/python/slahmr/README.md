---
title: Decoupling Human and Camera Motion from Videos in the Wild
python: https://github.com/rerun-io/slahmr
tags: [3D, SLAM, keypoint-detection, mesh, time-series]
thumbnail: https://static.rerun.io/43709757e7179f0272d3749560f529747f3e9149_slahmr_480w.png
---

SLAHMR robustly tracks the motion of multiple moving people filmed with a moving camera and works well on “in-the-wild” videos. It’s a great showcase of how to build working computer vision systems by intelligently combining several single purpose models.

https://www.youtube.com/watch?v=eGR4H0KkofA?playlist=eGR4H0KkofA&loop=1&hd=1&rel=0&autoplay=1

“Decoupling Human and Camera Motion from Videos in the Wild” (SLAHMR) combines the outputs of ViTPose, PHALP, DROID-SLAM, HuMoR, and SMPL over three optimization stages. It’s interesting to see how it becomes more and more consistent with each step.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/164c4ff3d203ccfe414a9a1d88a4054a16d6f9a9_slahmr_overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/96e558150be0814a1bafc6ee9be4d61a1c294975_slahmr_overview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/091c297b9f17339145ddf783934751ce2e1119bf_slahmr_overview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ef125c31b56021de74daa2932cb56f1e6a2ee8e8_slahmr_overview_1200w.png">
  <img src="https://static.rerun.io/e2a1716305c6e73107d5c54ae88fa5b2e9170acd_slahmr_overview_full.png" alt="">
</picture>

Input to the method is a video sequence. ViTPose is used to detect 2D skeletons, PHALP for 3D shape and pose estimation of the humans, and DROID-SLAM to estimate the camera trajectory. Note that the 3D poses are initially quite noisy and inconsistent.

https://www.youtube.com/watch?v=84hWddApYtI?playlist=84hWddApYtI&loop=1&hd=1&rel=0&autoplay=1

In the first stage, the 3D translation and rotation predicted by PHALP is optimized to better match the 2D keypoints from ViTPose. (left = before, right = after)

https://www.youtube.com/watch?v=iYy1sfDZsEc?playlist=iYy1sfDZsEc&loop=1&hd=1&rel=0&autoplay=1

In the second stage, in addition to 3D translation and rotation, the scale of the world, and the shape and pose of the bodies is optimized. To do so, in addition to the previous optimization term, a prior on joint smoothness, body shape, and body pose are added. (left = before, right = after)

https://www.youtube.com/watch?v=XXMKn29MlRI?playlist=XXMKn29MlRI&loop=1&hd=1&rel=0&autoplay=1

This step is crucial in that it finds the correct scale such that the humans don't drift in the 3D world. This can best be seen by overlaying the two estimates (the highlighted data is before optimization).

https://www.youtube.com/watch?v=FFHWNnZzUhA?playlist=FFHWNnZzUhA&loop=1&hd=1&rel=0&autoplay=1

Finally, in the third stage, a motion prior (HuMoR) is added to the optimization, and the ground plane is estimated to enforce realistic ground contact. This step further removes some jerky and unrealistic motions. Compare the highlighted blue figure. (left = before, right = after)

https://www.youtube.com/watch?v=6rsgOXekhWI?playlist=6rsgOXekhWI&loop=1&hd=1&rel=0&autoplay=1

For more details check out the [paper](https://arxiv.org/abs/2302.12827) by Vickie Ye, Georgios Pavlakos, Jitendra Malik, and Angjoo Kanazawa.
