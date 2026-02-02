<!--[metadata]
title = "RGBD"
tags = ["2D", "3D", "Depth", "NYUD", "Pinhole camera"]
thumbnail = "https://static.rerun.io/rgbd/2fde3a620adc8bd9a5680260f0792d16ac5498bd/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
build_args = ["--frames=300"]
-->

Visualizes an example recording from [the NYUD dataset](https://cs.nyu.edu/~fergus/datasets/nyu_depth_v2.html) with RGB and Depth channels.

<picture data-inline-viewer="examples/rgbd">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1200w.png">
  <img src="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/full.png" alt="RGBD example screenshot">
</picture>

## Used Rerun types

[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image)

## Background

The dataset, known as the NYU Depth V2 dataset, consists of synchronized pairs of RGB and depth frames recorded by the Microsoft Kinect in various indoor scenes.
This example visualizes one scene of this dataset, and offers a rich source of data for object recognition, scene understanding, depth estimation, and more.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Timelines

All data logged using Rerun in the following sections is connected to a specific time.
Rerun assigns a timestamp to each piece of logged data, and these timestamps are associated with a [`timeline`](https://www.rerun.io/docs/concepts/timelines).

```python
rr.set_time("time", timestamp=time.timestamp())
```

### Image

The example image is logged as [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) to the `world/camera/image/rgb` entity.

```python
rr.log("world/camera/image/rgb", rr.Image(img_rgb).compress(jpeg_quality=95))
```

### Depth image

Pinhole camera is utilized for achieving a 3D view and camera perspective through the use of the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole).

```python
rr.log(
    "world/camera/image",
    rr.Pinhole(
        resolution=[img_depth.shape[1], img_depth.shape[0]],
        focal_length=0.7 * img_depth.shape[1],
    ),
)
```

Then, the depth image is logged as a [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image) to the `world/camera/image/depth` entity.

```python
rr.log("world/camera/image/depth", rr.DepthImage(img_depth, meter=DEPTH_IMAGE_SCALING))
```

## Run the code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:

```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Install the necessary libraries specified in the requirements file:

```bash
pip install -e examples/python/rgbd
```

To experiment with the provided example, simply execute the main Python script:

```bash
python -m rgbd # run the example
```

You can specify the recording:

```bash
python -m rgbd --recording {cafe,basements,studies,office_kitchens,playroooms}
```

If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:

```bash
python -m rgbd --help
```
