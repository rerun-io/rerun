---
title = "Decoupling Human and Camera Motion from Videos in the Wild"
source = "https://github.com/rerun-io/slahmr"
tags = ["3D", "SLAM", "keypoint-detection", "mesh", "time-series"]
thumbnail = "https://static.rerun.io/slahmr/3fad4f6b2c1a807fb92e8d33a2f90f7391c290a2/480w.png"
thumbnail_dimensions = [480, 293]
---

SLAHMR robustly tracks the motion of multiple moving people filmed with a moving camera and works well on “in-the-wild” videos. It’s a great showcase of how to build working computer vision systems by intelligently combining several single purpose models.

https://vimeo.com/865974657?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

“Decoupling Human and Camera Motion from Videos in the Wild” (SLAHMR) combines the outputs of ViTPose, PHALP, DROID-SLAM, HuMoR, and SMPL over three optimization stages. It’s interesting to see how it becomes more and more consistent with each step.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/slahmr_overview/9e19834b2054b109d5093c1e5ffa0e7348ef3899/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/slahmr_overview/9e19834b2054b109d5093c1e5ffa0e7348ef3899/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/slahmr_overview/9e19834b2054b109d5093c1e5ffa0e7348ef3899/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/slahmr_overview/9e19834b2054b109d5093c1e5ffa0e7348ef3899/1200w.png">
  <img src="https://static.rerun.io/slahmr_overview/9e19834b2054b109d5093c1e5ffa0e7348ef3899/full.png" alt="">
</picture>

Input to the method is a video sequence. ViTPose is used to detect 2D skeletons, PHALP for 3D shape and pose estimation of the humans, and DROID-SLAM to estimate the camera trajectory. Note that the 3D poses are initially quite noisy and inconsistent.

https://vimeo.com/865974668?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

In the first stage, the 3D translation and rotation predicted by PHALP is optimized to better match the 2D keypoints from ViTPose. (left = before, right = after)

https://vimeo.com/865974684?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

In the second stage, in addition to 3D translation and rotation, the scale of the world, and the shape and pose of the bodies is optimized. To do so, in addition to the previous optimization term, a prior on joint smoothness, body shape, and body pose are added. (left = before, right = after)

https://vimeo.com/865974714?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

This step is crucial in that it finds the correct scale such that the humans don't drift in the 3D world. This can best be seen by overlaying the two estimates (the highlighted data is before optimization).

https://vimeo.com/865974747?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

Finally, in the third stage, a motion prior (HuMoR) is added to the optimization, and the ground plane is estimated to enforce realistic ground contact. This step further removes some jerky and unrealistic motions. Compare the highlighted blue figure. (left = before, right = after)

https://vimeo.com/865974760?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:6835

For more details check out the [paper](https://arxiv.org/abs/2302.12827) by Vickie Ye, Georgios Pavlakos, Jitendra Malik, and Angjoo Kanazawa.