<!--[metadata]
title = "EgoExo forge"
tags = ["3D", "HuggingFace", "Egocentric", "Exocentric", "manipulation"]
source = "https://github.com/rerun-io/egoexo-forge"
thumbnail = "https://static.rerun.io/egoexo_forge/629a093f1e2653711ad8fdd59c68b2318ca6bc6c/480w.png"
thumbnail_dimensions = [480, 436]
-->

https://vimeo.com/1134260310?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2386:1634

A collection of datasets and tools for egocentric and exocentric human activity understanding, featuring hand-object interactions, manipulation tasks, and multi-view recordings.

## Background

EgoExo Forge provides a consistent labeling scheme and data layout across multiple egocentric and exocentric human datasets with varying sensor configurations and annotations.

The following datasets are supported:

* [Assembly101](https://assembly-101.github.io/): A procedural activity dataset with 4321 multi-view videos of people assembling and disassembling 101 take-apart toy vehicles, featuring rich variations in action ordering, mistakes, and corrections.
* [HO-Cap](https://irvlutd.github.io/HOCap/): A dataset for 3D reconstruction and pose tracking of hands and objects in videos, featuring humans interacting with objects for various tasks including pick-and-place actions and handovers.
* [EgoDex](https://arxiv.org/abs/2505.11709): The largest and most diverse dataset of dexterous human manipulation with 829 hours of egocentric video and paired 3D hand tracking, covering 194 different tabletop tasks with everyday household objects.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/egoexo-forge) for more information.

You can try the example on a HuggingFace space [here](https://pablovela5620-egoexo-forge-viewer.hf.space/).

Or locally, make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run

```sh
git clone https://github.com/rerun-io/egoexo-forge.git
cd egoexo-forge
pixi run app
```
