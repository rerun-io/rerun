<!--[metadata]
title = "Open photogrammetry format"
tags = ["2D", "3D", "Camera", "Photogrammetry"]
thumbnail = "https://static.rerun.io/open-photogrammetry-format/c9bec43a3a3abd725a55ee8eb527a4c0cb01979b/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
build_args = ["--jpeg-quality=50"]
-->

Uses [`pyopf`](https://github.com/Pix4D/pyopf) to load and display a photogrammetrically reconstructed 3D point cloud in the [Open Photogrammetry Format (OPF)](https://www.pix4d.com/open-photogrammetry-format/).

<picture data-inline-viewer="examples/open_photogrammetry_format">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1200w.png">
  <img src="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/full.png" alt="">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole)

## Background

This example loads an Open Photogrammetry Format (OPF) project and displays the cameras and point cloud data.
OPF, which stands for 'open photogrammetry format,' is a file format used for photogrammetry data.
It contains all the necessary information related to a reconstructed 3D model made with photogrammetry, including calibration, point clouds and dense reconstruction.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Timelines

 For each processed frame, all data sent to Rerun is associated with specific time using [`timelines`](https://www.rerun.io/docs/concepts/timelines).

```python
rr.set_time("image", sequence=i)
```

### Video

Pinhole camera is utilized for achieving a 3D view and camera perspective through the use of the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes.

```python
rr.log(
    "world/cameras",
    rr.Transform3D(translation=calib_camera.position, mat3x3=rot)
)
```

```python
rr.log(
    "world/cameras/image",
    rr.Pinhole(
        resolution=sensor.image_size_px,
        focal_length=calib_sensor.internals.focal_length_px,
        principal_point=calib_sensor.internals.principal_point_px,
        camera_xyz=rr.ViewCoordinates.RUB,
    ),
)
```
The input video is logged as a sequence of [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) objects to the `world/cameras/image/rgb` entity.
```python
rr.log("world/cameras/image/rgb", rr.Image(np.array(img)).compress(jpeg_quality=jpeg_quality))
```

### Point clouds

Point clouds from the project are logged as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype to the `world/points` entity.

```python
rr.log("world/points", rr.Points3D(points.position, colors=points.color), static=True)
```


## Run the code


> This example requires Python 3.10 or higher because of [`pyopf`](https://pypi.org/project/pyopf/).

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/open_photogrammetry_format
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m open_photogrammetry_format # run the example
```
If you wish to customize it or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python -m open_photogrammetry_format --help
```
