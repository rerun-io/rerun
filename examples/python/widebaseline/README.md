<--[metadata]
title = "Learning to Render Novel Views from Wide-Baseline Stereo Pairs"
source = "https://github.com/rerun-io/cross_attention_renderer/"
tags = ["2D", "3D", "view-synthesis", "time-series", "pinhole-camera"]
thumbnail = "https://static.rerun.io/widebaseline/7bee6a2a13ede34f06a962019080d0dc102707b5/480w.png"
thumbnail_dimensions = [480, 316]
-->


Novel view synthesis has made remarkable progress in recent years, but most methods require per-scene optimization on many images. In their [CVPR 2023 paper](https://openaccess.thecvf.com/content/CVPR2023/html/Du_Learning_To_Render_Novel_Views_From_Wide-Baseline_Stereo_Pairs_CVPR_2023_paper.html) Yilun Du et al. propose a method that works with just 2 views. I created a visual walkthrough of the work using the Rerun SDK.

https://vimeo.com/865975229?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:10547

“Learning to Render Novel Views from Wide-Baseline Stereo Pairs” describes a three stage approach. (a) Image features for each input view are extracted. (b) Features along the target rays are collected. (c) The color is predicted through the use of cross-attention.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/widebaseline-overview/76d19a9bc9f4c101036577a747c029caa85fb95e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/widebaseline-overview/76d19a9bc9f4c101036577a747c029caa85fb95e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/widebaseline-overview/76d19a9bc9f4c101036577a747c029caa85fb95e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/widebaseline-overview/76d19a9bc9f4c101036577a747c029caa85fb95e/1200w.png">
  <img src="https://static.rerun.io/widebaseline-overview/76d19a9bc9f4c101036577a747c029caa85fb95e/full.png" alt="">
</picture>

To render a pixel its corresponding ray is projected onto each input image. Instead of uniformly sampling along the ray in 3D, the samples are distributed such that they are equally spaced on the image plane. The same points are also projected onto the other view (light color).

https://vimeo.com/865975245?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7941

The image features at these samples are used to synthesize new views. The method learns to attend to the features close to the surface. Here we show the attention maps for one pixel, and the resulting pseudo depth maps if we interpret the attention as a probability distribution.

https://vimeo.com/865975258?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:9184

Make sure to check out the [paper](https://openaccess.thecvf.com/content/CVPR2023/html/Du_Learning_To_Render_Novel_Views_From_Wide-Baseline_Stereo_Pairs_CVPR_2023_paper.html) by Yilun Du, Cameron Smith, Ayush Tewari, Vincent Sitzmann to learn about the details of the method.