<!--[metadata]
title = "Training a model on the LeRobot dataset"
tags = ["2D", "HuggingFace", "Imitation learning"]
source = "https://github.com/rerun-io/lerobot"
thumbnail = "https://static.rerun.io/lerobot-thumbnail/0462caa44339d4e74e01eef2b9206eebb585f6f8/480w.png"
thumbnail_dimensions = [480, 509]
-->

https://vimeo.com/983024799?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1920:1080

## Background

LeRobot is a project by huggingface that aims to provide models, datasets and tools for real-world robotics in PyTorch. This example shows how one can train a model on the [pusht-dataset](https://huggingface.co/datAsets/lerobot/pusht) and visualize it's progress using rerun.

## Run the code

This is an external example, check the [repository](https://github.com/rerun-io/lerobot/tree/alexander/train_viz) for more information.

To train the model as shown in the video, install git-lfs and clone the [repository](https://github.com/rerun-io/lerobot/tree/alexander/train_viz) and then run the following code:

```
pip install -e '.[pusht]'
WANDB_MODE=offline python lerobot/scripts/train.py \
  hydra.run.dir=outputs/train/diffusion_pusht \
  hydra.job.name=diffusion_pusht \
  policy=diffusion \
  env=pusht \
  env.task=PushT-v0 \
  dataset_repo_id=lerobot/pusht \
  training.offline_steps=20000 \
  training.save_freq=5000 ++training.log_freq=50 \
  training.eval_freq=1500 \
  eval.n_episodes=50 \
  wandb.enable=true \
  wandb.disable_artifact=true \
  device=cuda
```

If you don't have CUDA installed you will have to change the last argument `device=cuda` to `device=cpu` or another device.
