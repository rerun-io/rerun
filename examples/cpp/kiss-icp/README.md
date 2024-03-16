<!--[metadata]
title = "KISS-ICP"
tags = ["3D", "point-cloud"]
source = "https://github.com/rerun-io/kiss-icp"
description = "Visualizes the KISS-ICP LiDAR odometry pipeline on the NCLT dataset."
thumbnail = "https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/480w.png"
thumbnail_dimensions = [480, 288]
-->

Estimating the odometry is a common problem in robotics and in the [2023, "KISS-ICP: In Defense of Point-to-Point ICP -- Simple, Accurate, and Robust Registration If Done the Right Way" Ignacio Vizzo et al.](https://arxiv.org/abs/2209.15397) they show how one can use an ICP (iterative closest point) algorithm to robustly and accurately estimate poses from LiDAR data. We will demonstrate the KISS-ICP pipeline on the [NCLT dataset](http://robots.engin.umich.edu/nclt/) along with some brief explanations, for a more detailed explanation you should look at the [original paper](https://arxiv.org/abs/2209.15397).

<picture>
  <img src="https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/kiss-icp-screenshot/881ec7c7c0a0e50ec5d78d82875efaf3bb3c6e01/1200w.png">
</picture>

The KISS-ICP odometry pipeline consists of 4 steps. The first step is to compensate for movement of the vehicle during the LiDAR scan which is commonly called deskewing. This is done by creating an estimate of the rotational and translational velocity from previous pose estimates, then calculate the corrected scan under the assumption that the velocity is constant. In the screenshot below the raw scan is shown as green and the corrected scan is shown as the blue.

<picture>
  <img src="https://static.rerun.io/kiss-icp-deskewing/7157b0427d5358b18c5cf822669dc40601a1d4b6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/kiss-icp-deskewing/7157b0427d5358b18c5cf822669dc40601a1d4b6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/kiss-icp-deskewing/7157b0427d5358b18c5cf822669dc40601a1d4b6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/kiss-icp-deskewing/7157b0427d5358b18c5cf822669dc40601a1d4b6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/kiss-icp-deskewing/7157b0427d5358b18c5cf822669dc40601a1d4b6/1200w.png">
</picture>

The second step is to subsample the deskewed point cloud at two different resolutions. The first subsample with the highest resolution iis used to update the map after the pose has been estimated. The second is a subsample of the first subsample but with a lower resolution, this subsample is used during the ICP registration step. In the screenshot below the first subsample is colored pink and the second subsample is colored purple.

<picture>
  <img src="https://static.rerun.io/kiss-icp-subsampling/31eecf16f5f4d658a7391e051ead948cb0305913/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/kiss-icp-subsampling/31eecf16f5f4d658a7391e051ead948cb0305913/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/kiss-icp-subsampling/31eecf16f5f4d658a7391e051ead948cb0305913/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/kiss-icp-subsampling/31eecf16f5f4d658a7391e051ead948cb0305913/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/kiss-icp-subsampling/31eecf16f5f4d658a7391e051ead948cb0305913/1200w.png">
</picture>

The third step is to compute the adaptive threshold which will impose a limit on the distance between correspondences during the ICP. This is done using previously estimated poses to compute a likely limit of the displacement. The final step is to estimate the current pose by doing Point-to-Point ICP. This involves comparing each point in the subsampled scan with the closest point in the constructed map and making incremental updates to the estimated odometry based on these correspondences.

https://vimeo.com/923395317?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1920:1080
