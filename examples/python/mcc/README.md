---
title: "Single Image 3D Reconstruction using MCC, SAM, and ZoeDepth"
python: https://github.com/rerun-io/MCC
tags: [2D, 3D, segmentation, point-cloud, sam]
thumbnail: https://static.rerun.io/e62757c5953407373f2279be37a80748334cb6d7_mcc_480w.png
---

By combining MetaAI's [Segment Anything Model (SAM)](https://github.com/facebookresearch/segment-anything) and [Multiview Compressive Coding (MCC)](https://github.com/facebookresearch/MCC) we can get a 3D object from a single image.

https://www.youtube.com/watch?v=kmgFTWBZhWU?playlist=kmgFTWBZhWU&loop=1&hd=1&rel=0&autoplay=1

The basic idea is to use SAM to create a generic object mask so we can exclude the background.

https://www.youtube.com/watch?v=7qosqFbesL0?playlist=7qosqFbesL0&loop=1&hd=1&rel=0&autoplay=1

The next step is to generate a depth image. Here we use the awesome [ZoeDepth](https://github.com/isl-org/ZoeDepth) to get realistic depth from the color image.

https://www.youtube.com/watch?v=d0u-MoNVR6o?playlist=d0u-MoNVR6o&loop=1&hd=1&rel=0&autoplay=1

With depth, color, and an object mask we have everything needed to create a colored point cloud of the object from a single view

https://www.youtube.com/watch?v=LI0mE7usguk?playlist=LI0mE7usguk&loop=1&hd=1&rel=0&autoplay=1

MCC encodes the colored points and then creates a reconstruction by sweeping through the volume, querying the network for occupancy and color at each point.

https://www.youtube.com/watch?v=RuHv9Nx6PvI?playlist=RuHv9Nx6PvI&loop=1&hd=1&rel=0&autoplay=1

This is a really great example of how a lot of cool solutions are built these days; by stringing together more targeted pre-trained models.The details of the three building blocks can be found in the respective papers:
- [Segment Anything](https://arxiv.org/abs/2304.02643) by Alexander Kirillov, Eric Mintun, Nikhila Ravi, Hanzi Mao, Chloe Rolland, Laura Gustafson, Tete Xiao, Spencer Whitehead, Alexander C. Berg, Wan-Yen Lo, Piotr Dollár, and Ross Girshick
- [Multiview Compressive Coding for 3D Reconstruction](https://arxiv.org/abs/2301.08247) by Chao-Yuan Wu, Justin Johnson, Jitendra Malik, Christoph Feichtenhofer, and Georgia Gkioxari
- [ZoeDepth: Zero-shot Transfer by Combining Relative and Metric Depth](https://arxiv.org/abs/2302.12288) by Shariq Farooq Bhat, Reiner Birkl, Diana Wofk, Peter Wonka, and Matthias Müller
