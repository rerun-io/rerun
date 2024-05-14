<!--[metadata]
title = "Signed distance fields"
tags = ["3D", "mesh", "tensor"]
thumbnail = "https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/480w.png"
thumbnail_dimensions = [480, 294]
-->

Visualize the results of the Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103)

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/1200w.png">
  <img src="https://static.rerun.io/signed_distance_fields/99f6a886ed6f41b6a8e9023ba917a98668eaee70/full.png" alt="Signed Distance Fields example screenshot">
</picture>

## Used Rerun types
[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

## Background

This example illustrates the visualization of the results obtained from generating Signed Distance Fields (SDFs) for arbitrary meshes using both traditional methods and the approach described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103).
DeepSDF introduces a learned continuous representation of shapes using SDFs, enabling high-quality shape representation, interpolation, and completion from partial and noisy 3D input data.
This novel approach offers improved performance and reduced model size compared to previous methods.
The generated SDFs help with accurate 3D reconstruction and visualization.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### 3D asset

```python
# Internally, `mesh_to_sdf` will normalize everything to a unit sphere centered around the center of mass.
bs1 = mesh.bounding_sphere
bs2 = mesh_to_sdf.scale_to_unit_sphere(mesh).bounding_sphere
scale = bs2.scale / bs1.scale
center = bs2.center - bs1.center * scale
```

```python
# Logging the 3D asset with the unit sphere
mesh3d = rr.Asset3D(path=path)
mesh3d.transform = rr.OutOfTreeTransform3DBatch(rr.TranslationRotationScale3D(translation=center, scale=scale))
rr.log("world/mesh", mesh3d)
```

### Sample SDF

The sampled points and their corresponding signed distances are visualized using the [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype within the `world/sdf/points` entity.

```python
# Points inside the object are highlighted in red, while those outside are marked in green.
rr.log("world/sdf", rr.AnnotationContext([(0, "inside", (255, 0, 0)), (1, "outside", (0, 255, 0))]), timeless=False)
```

```python
rr.log("world/sdf/points", rr.Points3D(points, class_ids=np.array(sdf > 0, dtype=np.uint8))) # Visualizing Sample SDF
```

### Volumetric SDF

The computed distances for each voxel are visualized using the [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor) archetype to the `tensor` entity, which represents a 3D grid with dimensions for width, height, and depth.

```python
rr.log("tensor", rr.Tensor(voxvol, dim_names=["width", "height", "depth"])) # Visualizing Volumetric SDF
```

## Run the code
> _Known issue_: On macOS, this example may present artefacts in the SDF and/or fail.

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/signed_distance_fields
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m signed_distance_fields # run the example
```
You can specify the mesh:
```bash
python -m signed_distance_fields --mesh {lantern,avocado,buggy,brain_stem}
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m signed_distance_fields --help
```
