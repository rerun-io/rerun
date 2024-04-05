<!--[metadata]
title = "Signed distance fields"
tags = ["3D", "Mesh", "Tensor"]
thumbnail = "https://static.rerun.io/signed-distance-fields/0b0a200e0a5ec2b16e5f7d2da09b3ec6af3bac2d/480w.png"
thumbnail_dimensions = [480, 480]
-->

<picture>
  <img src="https://static.rerun.io/signed_distance_fields/1380524f963af0cbd615989a6e382ca86148f6da/full.png" alt="Signed Distance Fields example screenshot">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/signed_distance_fields/1380524f963af0cbd615989a6e382ca86148f6da/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/signed_distance_fields/1380524f963af0cbd615989a6e382ca86148f6da/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/signed_distance_fields/1380524f963af0cbd615989a6e382ca86148f6da/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/signed_distance_fields/1380524f963af0cbd615989a6e382ca86148f6da/1200w.png">
</picture>

Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103), and visualize the results using the Rerun SDK.

```bash
pip install -r examples/python/signed_distance_fields/requirements.txt
python examples/python/signed_distance_fields/main.py
```

_Known issue_: On macOS, this example may present artefacts in the SDF and/or fail.
