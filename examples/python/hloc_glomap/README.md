<!--[metadata]
title = "Hierarchical-Localization and GLOMAP"
tags = ["2D", "3D", "COLMAP", "Pinhole camera", "Time series", "GLOMAP", ]
source = "https://github.com/pablovela5620/hloc-glomap"
thumbnail = "https://static.rerun.io/thumbnail/6a5b887927834a6ea3db9474ce1e843ecd28b3cf/480w.png"
thumbnail_dimensions = [480, 300]
-->

https://vimeo.com/1037241347?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background

This examples allows use of the Hierarchical-Localization (hloc) repo and GLOMAP for easy and fast Structure-from-Motion with deep learned features and matchers. The Hierarchical-Localization repo (hloc for short) is a modular toolbox for state-of-the-art 6-DoF visual localization. It implements Hierarchical Localization, leveraging image retrieval and feature matching, and is fast, accurate, and scalable. This codebase combines and makes easily accessible years of research on image matching and Structure-from-Motion. GLOMAP is a general purpose global structure-from-motion pipeline for image-based sparse reconstruction. As compared to COLMAP it provides a much more efficient and scalable reconstruction process, typically 1-2 orders of magnitude faster, with on-par or superior reconstruction quality.

## Run the code

This is an external example. Check the [repository](https://github.com/pablovela5620/hloc-glomap) for more information on how to run the code.

TLDR: make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run
```
git clone https://github.com/pablovela5620/hloc-glomap.git
cd hloc-glomap
pixi run app
```
