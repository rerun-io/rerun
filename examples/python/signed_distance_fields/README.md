<--[metadata]
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

Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103), and visualize the results using the Rerun SDK.

```bash
pip install -r examples/python/signed_distance_fields/requirements.txt
python examples/python/signed_distance_fields/main.py
```

_Known issue_: On macOS, this example may present artefacts in the SDF and/or fail.