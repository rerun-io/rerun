<!--[metadata]
title = "Dicom MRI"
tags = ["tensor", "mri", "dicom"]
description = "Example using a DICOM MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer."
thumbnail = "https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/480w.png"
thumbnail_dimensions = [480, 285]
channel = "main"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/1200w.png">
  <img src="https://static.rerun.io/dicom_mri/e39f34a1b1ddd101545007f43a61783e1d2e5f8e/full.png" alt="">
</picture>

Visualize a [DICOM](https://en.wikipedia.org/wiki/DICOM) MRI scan. This demonstrates the flexible tensor slicing capabilities of the Rerun viewer.

## Used Rerun Types
[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

# Run the Code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/dicom_mri/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/dicom_mri/main.py # run the example
```

If you wish to customize it, explore additional features, or save it, use the CLI with the `--help` option for guidance:

```bash
python examples/python/dicom_mri/main.py --help

usage: main.py [-h] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Example using MRI scan data in the DICOM format.

optional arguments:
  -h, --help    show this help message and exit
  --headless    Don t show GUI
  --connect     Connect to an external viewer
  --serve       Serve a web viewer (WARNING: experimental feature)
  --addr ADDR   Connect to this ip:port
  --save SAVE   Save data to a .rrd file at this path
  -o, --stdout  Log data to standard output, to be piped into a Rerun Viewer
```