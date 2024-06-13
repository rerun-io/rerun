<!--[metadata]
title = "OCR"
tags = ["Text", "OCR", "2D", "Blueprint"]
thumbnail = "https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/480w.png"
thumbnail_dimensions = [480, 359]
# Channel = "main" # uncomment if this example can be run fast an easily
-->

This example visualizes layout analysis and text detection of documents.

<picture>
  <img src="https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ocr/259ba16f4d034a00baa6ecc0060849f7c13506b9/1200w.png">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`TextDocument`](https://rerun.io/docs/reference/types/archetypes/text_document), [`Boxes2D`](https://rerun.io/docs/reference/types/archetypes/boxes2d), [`AnnotationContext`](https://rerun.io/docs/reference/types/archetypes/annotation_context)

## Background
This example demonstrates the ability to visualize and verify the document layout analysis and text detection using the [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR).
[PP-Structure](https://github.com/PaddlePaddle/PaddleOCR/tree/main/ppstructure) used for this task, which is an intelligent document analysis system developed by the PaddleOCR team, which aims to help developers better complete tasks related to document understanding such as layout analysis and table recognition.
In the layout analysis task, the image first goes through the layout analysis model to divide the image into different areas such as text, table, figure and more, and then analyze these areas separately.
The classification of layouts and the text detection (including confidence levels) are visualized in the Rerun viewer. 
Finally, the recovery text document section presents the restored document with sorted order. By clicking on the restored text, the text area will be highlighted.

## Logging and visualizing with Rerun
The visualizations in this example were created with the following Rerun code.

### Image
The input document is logged as [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) object to the `Image` entity.
```python
rr.log("Image", rr.Image(coloured_image))
```

#### Label mapping 

An annotation context is logged with a class ID and a color assigned per layout type using [`AnnotationContext`](https://rerun.io/docs/reference/types/archetypes/annotation_context).

```python
class LayoutType(Enum):
    UNKNOWN = (0, "unknown", Color.Purple)
    TITLE = (1, "title", Color.Red)
    TEXT = (2, "text", Color.Green)
    FIGURE = (3, "figure", Color.Blue)
    FIGURE_CAPTION = (4, "figure_caption", Color.Yellow)
    TABLE = (5, "table", Color.Cyan)
    TABLE_CAPTION = (6, "table_caption", Color.Magenta)
    REFERENCE = (7, "reference", Color.Purple)
    FOOTER = (7, "footer", Color.Orange)
    
    @property
    def number(self):
        return self.value[0]  # Returns the numerical identifier

    @property
    def type(self):
        return self.value[1]  # Returns the type

    @property
    def color(self):
        return self.value[2]  # Returns the color
    
    @classmethod
    def get_annotation(cls):
        return [(layout.number, layout.type, layout.color) for layout in cls]


def detect_and_log_layout(img_path):
    rr.log(
        "Image",
        # The annotation is defined in the Layout class based on its properties
        rr.AnnotationContext(LayoutType.get_annotation()),
        timeless=True
    )
```

### Detections
The detections include the layout types and the text detections. Both of them are logged as [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d) to Rerun.

```python
rr.log(
    base_path,
    rr.Boxes2D(
        array=record['bounding_box'],
        array_format=rr.Box2DFormat.XYXY,
        labels=[str(layout_type.type)],
        class_ids=[str(layout_type.number)]
    ),
    rr.AnyValues(name=record_name),
    timeless=True
)
```
In the detection of the text, the detection id and the confidence are given supplementary.
```python
rr.log(
    f"{base_path}/Detections/{detection['id']}",
    rr.Boxes2D(
        array=detection['box'],
        array_format=rr.Box2DFormat.XYXY,
        class_ids=[str(layout_type.number)]
    ),
    rr.AnyValues(
        DetectionID=detection['id'],
        Text=detection['text'],
        Confidence=detection['confidence']
    ),
    timeless=True
)
```

### Setting up the blueprint

The blueprint for this example is created by the following code:

```python
rr.send_blueprint(rrb.Blueprint(
        rrb.Vertical(
            rrb.Horizontal(
                rrb.Spatial2DView(name="Layout", origin='Image/', contents=["Image/**"] + detections_paths),
                rrb.Spatial2DView(name="Detections", contents=["Image/**"]),
                rrb.TextDocumentView(name="Recovery", contents='Recovery')
            ),
            rrb.Horizontal(
                *tabs
            ),
            row_shares=[4, 3],
        ),
        collapse_panels=True,
    )
)
```

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
$ pip install --upgrade rerun-sdk  # install the latest Rerun SDK
$ git clone git@github.com:rerun-io/rerun.git  # Clone the repository
$ cd rerun
$ git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
$ pip install -e examples/python/ocr
```
To experiment with the provided example, simply execute the main Python script:
```bash
$ python -m ocr # run the example
```
If you wish to explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
$ python -m ocr --help
```
