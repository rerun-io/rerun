<!--[metadata]
title = "Dicom MRI"
tags = ["Tensor", "MRI", "DICOM"]
thumbnail = "https://static.rerun.io/dicom-mri/d5a434f92504e8dda8af6c7f4eded2a9d662c991/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
include_in_manifest = true
-->

Visualize a [DICOM](https://en.wikipedia.org/wiki/DICOM) MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer.

<picture data-inline-viewer="examples/dicom_mri">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1200w.png">
  <img src="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/full.png" alt="">
</picture>

## Used Rerun types
[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Background
Digital Imaging and Communications in Medicine (DICOM) serves as a technical standard for the digital storage and transmission of medical images. In this instance, an MRI scan is visualized using Rerun.

## Logging and visualizing with Rerun

The visualizations in this example were created with just the following line.
```python
rr.log("tensor", rr.Tensor(voxels_volume_u16, dim_names=["right", "back", "up"]))
```

A `numpy.array` named `voxels_volume_u16` representing volumetric MRI intensities with a shape of `(512, 512, 512)`.
To visualize this data effectively in Rerun, we can log the `numpy.array` as [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor) to the `tensor` entity.

In the Rerun Viewer you can also inspect the data in detail. The `dim_names` provided in the above call to `rr.log` help to
give semantic meaning to each axis. After selecting the tensor view, you can adjust various settings in the Blueprint
settings on the right-hand side. For example, you can adjust the color map, the brightness, which dimensions to show as
an image and which to select from, and more.

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
