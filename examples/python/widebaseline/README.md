---
title: "Learning to Render Novel Views from Wide-Baseline Stereo Pairs"
python: https://github.com/rerun-io/cross_attention_renderer/
tags: [2D, 3D, view-synthesis, time-series, pinhole-camera]
thumbnail: https://static.rerun.io/2bca79bebaf58c7f8780756e07b93798abe5f6d8_widebaseline_480w.png
thumbnail_dimensions: [480, 316]
---

Novel view synthesis has made remarkable progress in recent years, but most methods require per-scene optimization on many images. In their [CVPR 2023 paper](https://openaccess.thecvf.com/content/CVPR2023/html/Du_Learning_To_Render_Novel_Views_From_Wide-Baseline_Stereo_Pairs_CVPR_2023_paper.html) Yilun Du et al. propose a method that works with just 2 views. I created a visual walkthrough of the work using the Rerun SDK.

https://www.youtube.com/watch?v=dc445VtMj_4?playlist=dc445VtMj_4&loop=1&hd=1&rel=0&autoplay=1

“Learning to Render Novel Views from Wide-Baseline Stereo Pairs” describes a three stage approach. (a) Image features for each input view are extracted. (b) Features along the target rays are collected. (c) The color is predicted through the use of cross-attention.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/c010bf05e00119b4e955d857ed5442ac2d45b618_widebaseline-overview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ffb8d85f7f7ece2ac95ddaf6f1ee2e414460183c_widebaseline-overview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/653bc34a86336770d93e15b49f208369136c54e2_widebaseline-overview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/116562b999ccbb40b0340285c14454d356bb7982_widebaseline-overview_1200w.png">
  <img src="https://static.rerun.io/96903fa159c78d3afdc4ad56096d0caba0111e40_widebaseline-overview_full.png" alt="">
</picture>

To render a pixel its corresponding ray is projected onto each input image. Instead of uniformly sampling along the ray in 3D, the samples are distributed such that they are equally spaced on the image plane. The same points are also projected onto the other view (light color).

https://www.youtube.com/watch?v=PuoL94tBxGI?playlist=PuoL94tBxGI&loop=1&hd=1&rel=0&autoplay=1

The image features at these samples are used to synthesize new views. The method learns to attend to the features close to the surface. Here we show the attention maps for one pixel, and the resulting pseudo depth maps if we interpret the attention as a probability distribution.

https://www.youtube.com/watch?v=u-dmTM1w7Z4?playlist=u-dmTM1w7Z4&loop=1&hd=1&rel=0&autoplay=1

Make sure to check out the [paper](https://openaccess.thecvf.com/content/CVPR2023/html/Du_Learning_To_Render_Novel_Views_From_Wide-Baseline_Stereo_Pairs_CVPR_2023_paper.html) by Yilun Du, Cameron Smith, Ayush Tewari, Vincent Sitzmann to learn about the details of the method.
