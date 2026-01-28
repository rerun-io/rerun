<!--[metadata]
title = "Segment anything model"
tags = ["2D", "SAM", "Segmentation"]
thumbnail = "https://static.rerun.io/segment-anything-model/36438df27a287e5eff3a673e2464af071e665fdf/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
-->

Example of using Rerun to log and visualize the output of [Meta AI's Segment Anything model](https://github.com/facebookresearch/segment-anything).

<picture data-inline-viewer="examples/segment_anything_model">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/1200w.png">
  <img src="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/full.png" alt="Segment Anything Model example screenshot">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image), [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d)

## Background
This example showcases the visualization capabilities of [Meta AI's Segment Anything model](https://github.com/facebookresearch/segment-anything).
The visualization provided in this example demonstrates the precise and accurate segmentation capabilities of the model, effectively distinguishing each object from the background and creating a transparent mask around them.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Timelines

All data logged using Rerun in the following sections is connected to a specific frame.
Rerun assigns a frame to each piece of logged data, and these timestamps are associated with a [`timeline`](https://www.rerun.io/docs/concepts/timelines).

 ```python
for n, image_uri in enumerate(args.images):
    rr.set_time("image", sequence=n)
    image = load_image(image_uri)
    run_segmentation(mask_generator, image)
 ```

### Image
The input image is logged as [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) to the `image` entity.
```python
rr.log("image", rr.Image(image))
```
### Segmentation
All masks are stacked together and logged using the [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor) archetype.
```python
rr.log("mask_tensor", rr.Tensor(mask_tensor))
```
Then, all the masks are layered together and the result is logged as a [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) to the `image/masks` entity.
```python
rr.log("image/masks", rr.SegmentationImage(segmentation_img.astype(np.uint8)))
```
For object localization, bounding boxes of segmentations are logged as [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d).
```python
rr.log(
    "image/boxes",
    rr.Boxes2D(array=mask_bbox, array_format=rr.Box2DFormat.XYWH, class_ids=[id for id, _ in masks_with_ids]),
)
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
pip install -e examples/python/segment_anything_model
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m segment_anything_model # run the example
```
If you wish to customize it or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python -m segment_anything_model --help
```
