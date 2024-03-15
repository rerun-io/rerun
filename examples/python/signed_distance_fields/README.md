<!--[metadata]
title = "Signed Distance Fields"
tags = ["3D", "mesh", "tensor"]
thumbnail = "https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/480w.png"
thumbnail_dimensions = [480, 294]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/1200w.png">
  <img src="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/full.png" alt="Signed Distance Fields example screenshot">
</picture>

Visualise the results of the Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103)

## Used Rerun Types
[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

## Run the Code
_Known issue_: On macOS, this example may present artefacts in the SDF and/or fail.

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
pip install -r examples/python/signed_distance_fields/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/signed_distance_fields/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/signed_distance_fields/main.py --help 

usage: main.py [-h] [--resolution RESOLUTION] [--points POINTS] [--mesh {lantern,avocado,buggy,brain_stem}] [--mesh-path MESH_PATH] [--headless] [--connect]
               [--serve] [--addr ADDR] [--save SAVE] [-o]

Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK.

optional arguments:
  -h, --help            show this help message and exit
  --resolution RESOLUTION
                        Specifies the resolution of the voxel volume
  --points POINTS       Specifies the number of points for the point cloud
  --mesh {lantern,avocado,buggy,brain_stem}
                        The name of the mesh to analyze
  --mesh-path MESH_PATH
                        Path to a mesh to analyze. If set, overrides the `--mesh` argument.
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer
```

[//]: # (Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper]&#40;https://arxiv.org/abs/1901.05103&#41;, and visualize the results using the Rerun SDK.)

[//]: # (```bash)

[//]: # (pip install -r examples/python/signed_distance_fields/requirements.txt)

[//]: # (python examples/python/signed_distance_fields/main.py)

[//]: # (```)

[//]: # ()
