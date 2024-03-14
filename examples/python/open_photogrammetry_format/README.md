<!--[metadata]
title = "Open Photogrammetry Format"
tags = ["2D", "3D", "camera", "photogrammetry"]
description = "Displays a photogrammetrically reconstructed 3D point cloud loaded from an Open Photogrammetry Format (OPF) file."
thumbnail = "https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/480w.png"
thumbnail_dimensions = [480, 310]
channel = "release"
build_args = ["--jpeg-quality=50"]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/1200w.png">
  <img src="https://static.rerun.io/open_photogrammetry_format/603d5605f9670889bc8bce3365f16b831fce1eb1/full.png" alt="">
</picture>

Visualize a photogrammetrically reconstructed 3D point cloud loaded from an [Open Photogrammetry Format (OPF)](https://www.pix4d.com/open-photogrammetry-format/) file using [`pyopf`](https://github.com/Pix4D/pyopf) and Rerun.

[//]: # (Uses [`pyopf`]&#40;https://github.com/Pix4D/pyopf&#41; to load and display a photogrammetrically reconstructed 3D point cloud in the [Open Photogrammetry Format &#40;OPF&#41;]&#40;https://www.pix4d.com/open-photogrammetry-format/&#41;.)


## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), `ImageEncoded`, [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole)

# Run the Code
To run this example, make sure you have Python version at least 3.10 (because of [`pyopf`](https://pypi.org/project/pyopf/)), the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/open_photogrammetry_format/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/open_photogrammetry_format/main.py # run the example
```
If you wish to customize it or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python examples/python/open_photogrammetry_format/main.py --help 
```


