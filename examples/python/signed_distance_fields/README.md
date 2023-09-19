---
title: Signed Distance Fields
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/signed_distance_fields/main.py
tags:  [3D, mesh, tensor]
thumbnail: https://static.rerun.io/5d754551816a01c11726f005e7a02a39571a11a5_signed_distance_fields_480w.png
thumbnail_dimensions: [480, 294]
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/5d754551816a01c11726f005e7a02a39571a11a5_signed_distance_fields_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/f11337abddb58d9ed010f4a79267ac66984ee224_signed_distance_fields_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/372e9531c24e02027cc78333497d6fafedfd6916_signed_distance_fields_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/d84624aac1a0f9bc95a09729d68a6ef77072dd4f_signed_distance_fields_1200w.png">
  <img src="https://static.rerun.io/99f6a886ed6f41b6a8e9023ba917a98668eaee70_signed_distance_fields_full.png" alt="Signed Distance Fields example screenshot">
</picture>

Generate Signed Distance Fields for arbitrary meshes using both traditional methods and the one described in the [DeepSDF paper](https://arxiv.org/abs/1901.05103), and visualize the results using the Rerun SDK.

```bash
pip install -r examples/python/signed_distance_fields/requirements.txt
python examples/python/signed_distance_fields/main.py
```

_Known issue_: On macOS, this example may present artefacts in the SDF and/or fail.
