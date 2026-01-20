<!--[metadata]
title = "Detect and track objects"
tags = ["2D", "Hugging face", "Object detection", "Object tracking", "OpenCV"]
thumbnail = "https://static.rerun.io/detect-and-track-objects/63d7684ab1504c86a5375cb5db0fc515af433e08/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
allow_warnings = true # TODO(emilk): torch produces a warning because of `transformers` (I think?). We should fix that, if we can.
-->

Visualize object detection and segmentation using the [Huggingface's Transformers](https://huggingface.co/docs/transformers/index) and [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

<picture data-inline-viewer="examples/detect_and_track_objects">
  <img src="https://static.rerun.io/detact_and_track_objects/ce1939b8f2d22b36c4ca8b36dc0441e106b51da5/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/detact_and_track_objects/ce1939b8f2d22b36c4ca8b36dc0441e106b51da5/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/detact_and_track_objects/ce1939b8f2d22b36c4ca8b36dc0441e106b51da5/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/detact_and_track_objects/ce1939b8f2d22b36c4ca8b36dc0441e106b51da5/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/detact_and_track_objects/ce1939b8f2d22b36c4ca8b36dc0441e106b51da5/1200w.png">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`AssetVideo`](https://www.rerun.io/docs/reference/types/archetypes/asset_video), [`VideoFrameReference`](https://rerun.io/docs/reference/types/archetypes/video_frame_reference), [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

## Background
In this example, CSRT (Channel and Spatial Reliability Tracker), a tracking API introduced in OpenCV, is employed for object detection and tracking across frames.
Additionally, the example showcases basic object detection and segmentation on a video using the Huggingface transformers library.


## Logging and visualizing with Rerun
The visualizations in this example were created with the following Rerun code.


### Timelines
For each processed video frame, all data sent to Rerun is associated with the [`timelines`](https://www.rerun.io/docs/concepts/timelines) `frame_idx`.

```python
rr.set_time("frame", sequence=frame_idx)
```

### Video
The input video is logged as a static [`AssetVideo`](https://www.rerun.io/docs/reference/types/archetypes/asset_video) to the `video` entity.

```python
video_asset = rr.AssetVideo(path=video_path)
frame_timestamps_ns = video_asset.read_frame_timestamps_nanos()

rr.log("video", video_asset, static=True)
```

Each frame is processed and the timestamp is logged to the `frame` timeline using a [`VideoFrameReference`](https://www.rerun.io/docs/reference/types/archetypes/video_frame_reference).

```python
rr.log(
    "video",
    rr.VideoFrameReference(nanoseconds=frame_timestamps_ns[frame_idx])
)
```

Since the detection and segmentation model operates on smaller images the resized images are logged to the separate `segmentation/rgb_scaled` entity.
This allows us to subsequently visualize the segmentation mask on top of the video.

```python
rr.log(
    "segmentation/rgb_scaled",
    rr.Image(rgb_scaled).compress(jpeg_quality=85)
)
```

### Segmentations
The segmentation results is logged through a combination of two archetypes.
The segmentation image itself is logged as an
[`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image) and
contains the id for each pixel. It is logged to the `segmentation` entity.


```python
rr.log(
    "segmentation",
    rr.SegmentationImage(mask)
)
```

The color and label for each class is determined by the
[`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) which is
logged to the root entity using `rr.log("/", …, static=True)` as it should apply to the whole sequence and all
entities that have a class id.

```python
class_descriptions = [ rr.AnnotationInfo(id=cat["id"], color=cat["color"], label=cat["name"]) for cat in coco_categories ]
rr.log(
     "/",
     rr.AnnotationContext(class_descriptions),
     static=True
)
```

### Detections
The detections and tracked bounding boxes are visualized by logging the [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d) to Rerun.

#### Detections
```python
rr.log(
    "segmentation/detections/things",
    rr.Boxes2D(
        array=thing_boxes,
        array_format=rr.Box2DFormat.XYXY,
        class_ids=thing_class_ids,
    ),
)
```

```python
rr.log(
    f"image/tracked/{self.tracking_id}",
    rr.Boxes2D(
        array=self.tracked.bbox_xywh,
        array_format=rr.Box2DFormat.XYWH,
        class_ids=self.tracked.class_id,
    ),
)
```
#### Tracked bounding boxes
```python
rr.log(
    "segmentation/detections/background",
    rr.Boxes2D(
        array=background_boxes,
        array_format=rr.Box2DFormat.XYXY,
        class_ids=background_class_ids,
    ),
)
```

The color and label of the bounding boxes is determined by their class id, relying on the same
[`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context) as the
segmentation images. This ensures that a bounding box and a segmentation image with the same class id will also have the
same color.

Note that it is also possible to log multiple annotation contexts should different colors and / or labels be desired.
The annotation context is resolved by seeking up the entity hierarchy.

### Text log
Rerun integrates with the [Python logging module](https://docs.python.org/3/library/logging.html).
Through the [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log#textlogintegration) text at different importance level can be logged. After an initial setup that is described on the
[`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log#textlogintegration), statements
such as `logging.info("…")`, `logging.debug("…")`, etc. will show up in the Rerun viewer.

```python
def setup_logging() -> None:
    logger = logging.getLogger()
    rerun_handler = rr.LoggingHandler("logs")
    rerun_handler.setLevel(-1)
    logger.addHandler(rerun_handler)

def main() -> None:
    # … existing code …
    setup_logging() # setup logging
    track_objects(video_path, max_frame_count=args.max_frame) # start tracking
```
In the Viewer you can adjust the filter level and look at the messages time-synchronized with respect to other logged data.

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
pip install -e examples/python/detect_and_track_objects
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m detect_and_track_objects # run the example
```

If you wish to customize it for various videos, adjust the maximum frames, explore additional features, or save it use the CLI with the `--help` option for guidance:

```bash
python -m detect_and_track_objects --help
```
