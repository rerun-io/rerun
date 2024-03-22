<!--[metadata]
title = "Objectron"
tags = ["2D", "3D", "object-detection", "pinhole-camera"]
description = "Example of using the Rerun SDK to log the Google Research Objectron dataset."
thumbnail = "https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png"
thumbnail_dimensions = [480, 268]
# channel = "release"  - Disabled because it sometimes have bad first-frame heuristics
build_args = ["--frames=150"]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1200w.png">
  <img src="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/full.png" alt="Objectron example screenshot">
</picture>


Visualize the [Google Research Objectron](https://github.com/google-research-datasets/Objectron) dataset, which contains camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

## Used Rerun Types
 [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Image`](https://ref.rerun.io/docs/python/0.14.1/common/image_helpers/#rerun.ImageEncoded)<sup>*</sup>, [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole)

# Logging and Visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

## Timelines

For each processed frame, all data sent to Rerun is associated with the two [`timelines`](https://www.rerun.io/docs/concepts/timelines) `time` and `frame_idx`.

```python
rr.set_time_sequence("frame", sample.index)
rr.set_time_seconds("time", sample.timestamp)
```

## Video

Pinhole camera is utilized for achieving a 3D view and camera perspective through the use of the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes.

```python
rr.log(
        "world/camera",
        rr.Transform3D(translation=translation, rotation=rr.Quaternion(xyzw=rot.as_quat())),
)
```

```python
rr.log(
    "world/camera",
    rr.Pinhole(
        resolution=[w, h],
        image_from_camera=intrinsics,
        camera_xyz=rr.ViewCoordinates.RDF,
    ),
)
```
The input video is logged as a sequence of [`ImageEncoded`](https://ref.rerun.io/docs/python/0.14.1/common/image_helpers/#rerun.ImageEncoded) objects to the `world/camera` entity.
```python
rr.log("world/camera", rr.ImageEncoded(path=sample.image_path))
```

## Sparse Point Clouds

Sparse point clouds from `ARFrame` are logged as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype to the `world/points` entity.

```python
rr.log("world/points", rr.Points3D(positions, colors=[255, 255, 255, 255]))
```

## Annotated Bounding Boxes

Bounding boxes annotated from `ARFrame` are logged as [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), containing details such as object position, sizes, center and rotation.

```python
rr.log(
    f"world/annotations/box-{bbox.id}",
    rr.Boxes3D(
        half_sizes=0.5 * np.array(bbox.scale),
        centers=bbox.translation,
        rotations=rr.Quaternion(xyzw=rot.as_quat()),
        colors=[160, 230, 130, 255],
        labels=bbox.category,
    ),
    timeless=True,
)
```

# Run the Code
To run this example, make sure you have Python version at least 3.9, the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/objectron/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/objectron/main.py # run the example
```

You can specify the objectron recording:
```bash
python examples/python/objectron/main.py --recording {bike,book,bottle,camera,cereal_box,chair,cup,laptop,shoe}
```

If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/objectron/main.py --help 
```
