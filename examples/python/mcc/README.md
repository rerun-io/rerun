<--[metadata]
title = "Single Image 3D Reconstruction using MCC, SAM, and ZoeDepth"
source = "https://github.com/rerun-io/MCC"
tags = ["2D", "3D", "segmentation", "point-cloud", "sam"]
thumbnail = "https://static.rerun.io/mcc/d244be2806b5abcc0e905a2c262b491b73914658/480w.png"
thumbnail_dimensions = [480, 274]
-->


By combining MetaAI's [Segment Anything Model (SAM)](https://github.com/facebookresearch/segment-anything) and [Multiview Compressive Coding (MCC)](https://github.com/facebookresearch/MCC) we can get a 3D object from a single image.


https://vimeo.com/865973817?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:8133

The basic idea is to use SAM to create a generic object mask so we can exclude the background.


https://vimeo.com/865973836?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7941

The next step is to generate a depth image. Here we use the awesome [ZoeDepth](https://github.com/isl-org/ZoeDepth) to get realistic depth from the color image.


https://vimeo.com/865973850?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:7941

With depth, color, and an object mask we have everything needed to create a colored point cloud of the object from a single view


https://vimeo.com/865973862?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=10000:11688

MCC encodes the colored points and then creates a reconstruction by sweeping through the volume, querying the network for occupancy and color at each point.


https://vimeo.com/865973880?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1:1

This is a really great example of how a lot of cool solutions are built these days; by stringing together more targeted pre-trained models.The details of the three building blocks can be found in the respective papers:
- [Segment Anything](https://arxiv.org/abs/2304.02643) by Alexander Kirillov, Eric Mintun, Nikhila Ravi, Hanzi Mao, Chloe Rolland, Laura Gustafson, Tete Xiao, Spencer Whitehead, Alexander C. Berg, Wan-Yen Lo, Piotr Dollár, and Ross Girshick
- [Multiview Compressive Coding for 3D Reconstruction](https://arxiv.org/abs/2301.08247) by Chao-Yuan Wu, Justin Johnson, Jitendra Malik, Christoph Feichtenhofer, and Georgia Gkioxari
- [ZoeDepth: Zero-shot Transfer by Combining Relative and Metric Depth](https://arxiv.org/abs/2302.12288) by Shariq Farooq Bhat, Reiner Birkl, Diana Wofk, Peter Wonka, and Matthias Müller