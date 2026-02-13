<!--[metadata]
title = "LeRobot loader"
tags = ["2D", "Video", "Loader", "Hugging Face", "LeRobot"]
thumbnail = "https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/480w.png"
thumbnail_dimensions = [480, 275]
-->

<picture>
  <img src="https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/LeRobot/ec638243c38d01c0d9e27ef4e52e62c43c6e4ba4/1200w.png">
</picture>

## Overview

Rerun has a built in data-loader to visualize [LeRobot](https://github.com/huggingface/lerobot) datasets.

## Try it out

Here is a sample dataset used in [SmolVLA](https://huggingface.co/blog/smolvla)

```bash
git lfs install # If not already installed
git clone https://huggingface.co/datasets/satvikahuja/mixer_on_off_new_1
```

Then you can open the Viewer and select the directory to load the dataset, or you can open it directly from the terminal:

```bash
rerun mixer_on_off_new_1
```

### SDK support

Since this dataloader is included other SDK functionalities should work similar to loading a rerun file but pointing at the directory instead.
