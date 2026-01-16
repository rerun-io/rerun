<!--[metadata]
title = "Eye control"
tags = ["Eye control", "3D", "Pinhole camera"]
source = "https://github.com/rerun-io/eye_control_example"
thumbnail = "https://static.rerun.io/eye_control_example/01288e2cd92ec68715258e281104701fc8908c37/480w.png"
thumbnail_dimensions = [480, 306]
-->

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/d75d85d16b70cf6e7863367cd716c10e5725f852_eye_control_example_kth_rpl.mp4" type="video/mp4" />
</video>

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/7ef9a59523051d45c25de02e6a844e7179e205d2_eye_control_example_ntu_viral.mp4" type="video/mp4" />
</video>

## Used Rerun types

[`EyeControls3D`](https://ref.rerun.io/docs/python/0.27.3/common/blueprint_archetypes/#rerun.blueprint.archetypes.EyeControls3D),
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`EncodedImage`](https://www.rerun.io/docs/reference/types/archetypes/encoded_image), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`LineStrips3D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Background

This example demonstrates how to programmatically configure and control the 3D view camera using the Rerun Blueprint API. By defining camera states in Python, you can precisely tailor your workspace to highlight the most relevant aspects of your data.

In this example, we define several specialized perspectives:

* Top-down overview: a global scene perspective for general spatial awareness.
* Comparative close-up: A focused view designed to analyze trajectory deviations between different localization methods.
* 3rd-person follow:  dynamic camera that tracks the ego vehicle as it moves through the environment.

Finally, we demonstrate how to control the camera at runtime, enabling the creation of cinematic visualizations or automated data storytelling for presentations and datasets.

## Useful resources

Below you will find a collection of useful Rerun resources for this example:

* [Blueprints](https://rerun.io/docs/concepts/blueprints)
* [Building blueprints programmatically](https://rerun.io/docs/howto/build-a-blueprint-programmatically)

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/eye_control_example) for more information.

To run this example, make sure you have the [Pixi](https://pixi.sh/latest/#installation) package manager installed.

### KTH RPL (indoor handheld dataset)

```sh
pixi run kth_rpl
```

You can type:

```sh
pixi run kth_rpl -h
```

to see all available commands. For example, you can set the voxel size used for downsampling, where the dataset is located, and for how long to sleep in-between frames.

### NTU VIRAL (drone dataset)

```sh
pixi run ntu_viral
```
