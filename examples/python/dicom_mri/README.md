<!--[metadata]
title = "Dicom MRI"
description = "Visualize a DICOM MRI scan with tensor slicing and 3D volume raymarching."
tags = ["Tensor", "MRI", "DICOM"]
thumbnail = "https://static.rerun.io/dicom-thumbnail/81986d5b9ad8bae75c60f74333cb3468c1da7003/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
include_in_manifest = true
-->

Visualize a [DICOM](https://en.wikipedia.org/wiki/DICOM) MRI scan with the tensor slicing tools and a raymarched 3D volume.

<picture>
  <img src="https://static.rerun.io/dicom/1722e458d3d83ed309e6d239e1d726808f99db8c/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dicom/1722e458d3d83ed309e6d239e1d726808f99db8c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dicom/1722e458d3d83ed309e6d239e1d726808f99db8c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dicom/1722e458d3d83ed309e6d239e1d726808f99db8c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dicom/1722e458d3d83ed309e6d239e1d726808f99db8c/1200w.png">
</picture>

## Used Rerun types
[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`Volume3D`](../../../docs/content/reference/types/archetypes/volume3d.md), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Background
Digital Imaging and Communications in Medicine (DICOM) serves as a technical standard for the digital storage and transmission of medical images.
In this instance, an MRI scan is visualized using Rerun.

## Logging and visualizing with Rerun

The sliceable tensor and raymarched volume use the same logged `Volume3D:values` component.
```python
voxels_volume, ijk_to_xyz = extract_voxel_data(dicom_files)
voxels_volume_f16 = voxels_volume.T.astype(np.float16)
values = rr.TensorData(array=voxels_volume_f16, dim_names=["up", "back", "right"])
rr.log(
    "volume",
    rr.Transform3D(translation=ijk_to_xyz[:3, 3], mat3x3=ijk_to_xyz[:3, :3]),
    static=True,
)
rr.log("volume", rr.Volume3D(values, optical_density=0.5))
```

The voxel data is transposed to the `[z, y, x]` order expected by [`Volume3D`](../../../docs/content/reference/types/archetypes/volume3d.md), cast to `float16`, and logged once to the `volume` entity without any further normalization.
The Tensor view maps `Volume3D:values` to the [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor) visualizer's `Tensor:data` component.
Both visualizers therefore read the same stored tensor data.

The blueprint arranges the description and sliceable tensor vertically on the left, with the 3D volume on the right.
It configures the component mapping as a per-entity override in the Tensor view:

```python
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.encodings import ComponentSourceKind, VisualizerComponentMapping

tensor_visualizer = rr.Tensor.from_fields().visualizer(
    mappings=[
        VisualizerComponentMapping(
            target="Tensor:data",
            source_kind=ComponentSourceKind.SourceComponent,
            source_component="Volume3D:values",
        )
    ]
)
rr.send_blueprint(
    rrb.Horizontal(
        rrb.Vertical(
            rrb.TextDocumentView(name="Description", origin="/description"),
            rrb.TensorView(
                name="MRI slices",
                overrides={"/volume": tensor_visualizer},
            ),
        ),
        rrb.Spatial3DView(name="MRI volume"),
    )
)
```

`Horizontal` and `Vertical` control the view layout, while the description view's `origin` selects the `/description` entity containing the logged Markdown.
`source_component` selects the logged volume values, while `target` identifies the input expected by the Tensor visualizer.
`SourceComponent` tells the visualizer to read that component from the entity being visualized.
The `overrides` entry applies this visualizer configuration to `/volume` only in the Tensor view; the 3D view renders the same entity as a volume.
Both visualizers infer their value ranges from the voxel data.

In the Rerun Viewer you can also inspect the data in detail.
The `dim_names` provided when constructing `TensorData` give semantic meaning to each axis.
After selecting the tensor view, you can adjust various settings in the Blueprint settings on the right-hand side.
For example, you can adjust the color map, the brightness, which dimensions to show as an image and which to select from, and more.

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
pip install -e examples/python/dicom_mri
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m dicom_mri # run the example
```

If you wish to customize it, explore additional features, or save it, use the CLI with the `--help` option for guidance:

```bash
python -m dicom_mri --help
```
